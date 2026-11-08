//! `kel-native`, the native deployment shape as a command (V0.3.x Order 6).
//!
//! ```text
//! kel-native build <source.kel> --prefix <name> --out <dir>
//!                  [--target <triple>] [--archive] [--shared] [--fallible <native>]...
//!                  [--compiler <rust|self-hosted|self-hosted-native>]
//! kel-native inspect <source.kel> [--target <triple>] [--fallible <native>]...
//!                  [--compiler <rust|self-hosted|self-hosted-native>]
//! ```
//!
//! `build` lowers the source's entry as a coroutine library through
//! `keleusma_native::linkage` and writes `<prefix>.o`, `<prefix>.h` and
//! `<prefix>.rs` into the output directory. `--archive` adds `lib<prefix>.a`
//! with the platform `ar`, and `--shared` adds a shared object with the
//! platform `cc`, for a host that replaces the library while it runs.
//! `--fallible` names a native, as `module::name`, that may fail softly.
//!
//! `inspect` builds the same library in memory and prints its contract, the
//! trap verdict, the cost claim, the stack bound and the kind census, so a user
//! can see what the native shape will promise before writing anything.
//!
//! `--compiler` selects the compiler that produces the module the native route
//! lowers, which is question 19's selector. `rust`, the default, is the
//! reference compiler. `self-hosted` is the Keleusma-written pipeline on the
//! virtual machine, and `self-hosted-native` is the same pipeline with every
//! stage it can lower run as native code through
//! [`keleusma_native::stage_factory::NativeStageFactory`]. Both self-hosted
//! routes accept only the self-hosted subset, compile for the host's widths,
//! and are cross-checked against the reference compiler by
//! `keleusma::selfhost::self_hosted_compile`, so a divergence fails rather
//! than producing a module. A divergence shows that the two disagree, not which
//! one is wrong. A stage the native factory declines runs on the virtual
//! machine, and the command reports how many did, so a fully native compile is
//! visible as zero declined.
//!
//! # Exit status
//!
//! Zero on success, 64 for a usage error, 65 for a source the reference
//! compiler or the self-hosted pipeline rejects, 66 for a source that cannot
//! be read, 70 for a module the native route refuses, 71 for a failure to
//! write or to run `ar` or `cc`, and 72 for a source the reference compiler
//! accepts and a selected self-hosted compiler cannot compile, with a hint to
//! retry with `--compiler rust`. Every failure prints its reason on standard
//! error.

use inkwell::OptimizationLevel;
use inkwell::context::Context;
use inkwell::targets::{
    CodeModel, InitializationConfig, RelocMode, Target, TargetMachine, TargetTriple,
};
use keleusma_native::coroutine::{self, HostContracts};
use keleusma_native::linkage;
use keleusma_native::stage_factory::NativeStageFactory;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::rc::Rc;

const USAGE: &str = "usage:
  kel-native build <source.kel> --prefix <name> --out <dir> [--target <triple>] [--archive] [--shared] [--fallible <native>]... [--compiler <rust|self-hosted|self-hosted-native>]
  kel-native inspect <source.kel> [--target <triple>] [--fallible <native>]... [--compiler <rust|self-hosted|self-hosted-native>]";

/// The compiler that produces the module the native route lowers.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Compiler {
    /// The Rust reference compiler.
    Rust,
    /// The self-hosted pipeline with every stage on the virtual machine.
    SelfHosted,
    /// The self-hosted pipeline with its stages run as native code where they lower.
    SelfHostedNative,
}

impl Compiler {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "rust" => Some(Self::Rust),
            "self-hosted" => Some(Self::SelfHosted),
            "self-hosted-native" => Some(Self::SelfHostedNative),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::SelfHosted => "self-hosted",
            Self::SelfHostedNative => "self-hosted-native",
        }
    }
}

/// The compiled module, the program it was parsed from, and, for the native
/// self-hosted route, how many stages ran natively and how many were declined.
struct Compiled {
    module: keleusma::bytecode::Module,
    program: keleusma::ast::Program,
    stages: Option<(usize, usize)>,
}

/// A failure and the exit status it maps to.
struct Failure(u8, String);

type Outcome<T> = Result<T, Failure>;

/// The parsed command line.
struct Args {
    command: String,
    source: PathBuf,
    prefix: Option<String>,
    out: Option<PathBuf>,
    target: Option<String>,
    archive: bool,
    shared: bool,
    fallible: Vec<String>,
    compiler: Compiler,
}

fn usage(why: &str) -> Failure {
    Failure(64, format!("{why}\n{USAGE}"))
}

fn parse_args(raw: &[String]) -> Outcome<Args> {
    let mut it = raw.iter();
    let command = it.next().ok_or_else(|| usage("no command"))?.clone();
    if command != "build" && command != "inspect" {
        return Err(usage(&format!("unknown command `{command}`")));
    }
    let mut args = Args {
        command,
        source: PathBuf::new(),
        prefix: None,
        out: None,
        target: None,
        archive: false,
        shared: false,
        fallible: Vec::new(),
        compiler: Compiler::Rust,
    };
    let mut source = None;
    while let Some(a) = it.next() {
        let mut value = |flag: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| usage(&format!("{flag} needs a value")))
        };
        match a.as_str() {
            "--prefix" => args.prefix = Some(value("--prefix")?),
            "--out" => args.out = Some(PathBuf::from(value("--out")?)),
            "--target" => args.target = Some(value("--target")?),
            "--fallible" => args.fallible.push(value("--fallible")?),
            "--archive" => args.archive = true,
            "--shared" => args.shared = true,
            "--compiler" => {
                let name = value("--compiler")?;
                args.compiler = Compiler::parse(&name).ok_or_else(|| {
                    usage(&format!(
                        "unknown compiler `{name}`, expected rust, self-hosted or self-hosted-native"
                    ))
                })?;
            }
            flag if flag.starts_with("--") => return Err(usage(&format!("unknown flag `{flag}`"))),
            path if source.is_none() => source = Some(PathBuf::from(path)),
            extra => return Err(usage(&format!("unexpected argument `{extra}`"))),
        }
    }
    args.source = source.ok_or_else(|| usage("no source file"))?;
    if args.command == "build" && (args.prefix.is_none() || args.out.is_none()) {
        return Err(usage("build needs --prefix and --out"));
    }
    if args.command == "inspect" && (args.archive || args.shared || args.out.is_some()) {
        return Err(usage("inspect writes nothing, so it takes no output flags"));
    }
    Ok(args)
}

fn machine(target: Option<&str>) -> Outcome<TargetMachine> {
    let triple = match target {
        Some(t) => {
            Target::initialize_all(&InitializationConfig::default());
            TargetTriple::create(t)
        }
        None => {
            Target::initialize_native(&InitializationConfig::default())
                .map_err(|e| Failure(70, format!("native target: {e}")))?;
            TargetMachine::get_default_triple()
        }
    };
    Target::from_triple(&triple)
        .map_err(|e| {
            Failure(
                64,
                format!("target `{}`: {e}", triple.as_str().to_string_lossy()),
            )
        })?
        .create_target_machine(
            &triple,
            "generic",
            "",
            OptimizationLevel::Default,
            RelocMode::PIC,
            CodeModel::Default,
        )
        .ok_or_else(|| Failure(70, String::from("the target machine could not be created")))
}

/// The module the selected compiler produces, and the program parsed by the
/// reference front end, which supplies the opaque type names the module does
/// not record. The parse runs for every compiler, so a source the reference
/// front end rejects fails with 65 before a self-hosted route is tried.
fn compile(path: &Path, compiler: Compiler) -> Outcome<Compiled> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| Failure(66, format!("{}: {e}", path.display())))?;
    let tokens = keleusma::lexer::tokenize(&src).map_err(|e| Failure(65, format!("lex: {e:?}")))?;
    let program =
        keleusma::parser::parse(&tokens).map_err(|e| Failure(65, format!("parse: {e:?}")))?;
    let host = keleusma::target::Target::host();
    let self_hosted = |src: &str| {
        keleusma::selfhost::self_hosted_compile(src, &host).map_err(|e| {
            if e.rust_backend_would_help() {
                Failure(
                    72,
                    format!(
                        "{} compile: {e}; retry with --compiler rust",
                        compiler.name()
                    ),
                )
            } else {
                Failure(65, format!("compile: {e}"))
            }
        })
    };
    let (module, stages) = match compiler {
        Compiler::Rust => (
            keleusma::compiler::compile(&program)
                .map_err(|e| Failure(65, format!("compile: {e:?}")))?,
            None,
        ),
        Compiler::SelfHosted => (self_hosted(&src)?, None),
        Compiler::SelfHostedNative => {
            let factory = Rc::new(NativeStageFactory::default());
            let module =
                keleusma::selfhost::with_stage_factory(factory.clone(), || self_hosted(&src))?;
            (module, Some((factory.built(), factory.declined())))
        }
    };
    Ok(Compiled {
        module,
        program,
        stages,
    })
}

/// Report the native self-hosted route's stage counts on standard error, so
/// that the library's own report on standard output keeps its shape.
fn report_stages(stages: Option<(usize, usize)>) {
    if let Some((built, declined)) = stages {
        eprintln!("kel-native: compiled with {built} native stages, {declined} declined");
    }
}

fn contracts(
    module: &keleusma::bytecode::Module,
    program: &keleusma::ast::Program,
    fallible: &[String],
) -> Outcome<HostContracts> {
    let mut set = BTreeSet::new();
    for name in fallible {
        let index = module
            .native_names
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| usage(&format!("`{name}` is not a native this module calls")))?;
        set.insert(u16::try_from(index).map_err(|_| usage("native table too large"))?);
    }
    Ok(HostContracts {
        fallible_natives: set,
        opaque_natives: keleusma_native::opaque::opaque_natives(program, module),
        ..HostContracts::default()
    })
}

/// Run an external tool, mapping any failure to exit status 71.
fn tool(name: &str, args: &mut Command) -> Outcome<()> {
    let out = args
        .output()
        .map_err(|e| Failure(71, format!("could not run `{name}`: {e}")))?;
    if !out.status.success() {
        return Err(Failure(
            71,
            format!(
                "`{name}` failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        ));
    }
    Ok(())
}

fn build(args: &Args) -> Outcome<()> {
    let Compiled {
        module,
        program,
        stages,
    } = compile(&args.source, args.compiler)?;
    report_stages(stages);
    let tm = machine(args.target.as_deref())?;
    let host = contracts(&module, &program, &args.fallible)?;
    let prefix = args.prefix.as_deref().expect("checked by parse_args");
    let out = args.out.as_deref().expect("checked by parse_args");
    let ctx = Context::create();
    let lib = linkage::build(&ctx, &module, &tm, prefix, &host)
        .map_err(|e| Failure(70, format!("the native route refused it: {e:?}")))?;
    std::fs::create_dir_all(out).map_err(|e| Failure(71, format!("{}: {e}", out.display())))?;
    let object = out.join(format!("{prefix}.o"));
    lib.write_object(&tm, &object)
        .map_err(|e| Failure(71, format!("write object: {e:?}")))?;
    for (name, text) in [
        (format!("{prefix}.h"), lib.c_header()),
        (format!("{prefix}.rs"), lib.rust_bindings()),
    ] {
        std::fs::write(out.join(&name), text).map_err(|e| Failure(71, format!("{name}: {e}")))?;
    }
    if args.archive {
        let ar = std::env::var("AR").unwrap_or_else(|_| String::from("ar"));
        let archive = out.join(format!("lib{prefix}.a"));
        let _ = std::fs::remove_file(&archive);
        tool(&ar, Command::new(&ar).arg("rcs").arg(&archive).arg(&object))?;
    }
    if args.shared {
        let cc = std::env::var("CC").unwrap_or_else(|_| String::from("cc"));
        let (flag, ext) = if cfg!(target_os = "macos") {
            ("-dynamiclib", "dylib")
        } else {
            ("-shared", "so")
        };
        let shared = out.join(format!("lib{prefix}.{ext}"));
        tool(
            &cc,
            Command::new(&cc)
                .arg(flag)
                .arg(&object)
                .arg("-o")
                .arg(&shared),
        )?;
    }
    println!(
        "{prefix}: frame {} B, slot {} B, stack {} B, wcet status {}, trap verdict {}",
        lib.frame_bytes,
        lib.slot_bytes,
        lib.stack_bytes,
        lib.wcet.status(),
        lib.trap.code()
    );
    Ok(())
}

fn inspect(args: &Args) -> Outcome<()> {
    let Compiled {
        module,
        program,
        stages,
    } = compile(&args.source, args.compiler)?;
    report_stages(stages);
    let tm = machine(args.target.as_deref())?;
    let host = contracts(&module, &program, &args.fallible)?;
    let ctx = Context::create();
    let lib = linkage::build(&ctx, &module, &tm, "kel_inspect", &host)
        .map_err(|e| Failure(70, format!("the native route refused it: {e:?}")))?;
    let census = coroutine::kind_census(&module, &host)
        .map_err(|e| Failure(70, format!("kind census: {e}")))?;
    println!("frame bytes        {}", lib.frame_bytes);
    println!("slot bytes         {}", lib.slot_bytes);
    println!("shared bytes       {}", lib.shared_bytes);
    println!("private bytes      {}", lib.private_bytes);
    println!("region bytes       {}", lib.region_bytes);
    println!("stack bytes        {}", lib.stack_bytes);
    println!("schema hash        {:#x}", lib.schema_hash);
    println!("trap verdict       {}", lib.trap.code());
    match &lib.wcet {
        linkage::CostExport::Established(c) => {
            println!("cost claim         {c} bytecode cost units, best effort as native time")
        }
        linkage::CostExport::Unestablished(why) => {
            println!("cost claim         not established: {why}")
        }
    }
    match &census.first_dynamic {
        None => println!("run-time kinds     none, the code is tag-free"),
        Some(why) => println!("run-time kinds     needed, first {why}"),
    }
    for n in &lib.natives {
        println!(
            "native             {}{}",
            n.symbol,
            if n.fallible { " (fallible)" } else { "" }
        );
    }
    Ok(())
}

fn main() -> ExitCode {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let result = parse_args(&raw).and_then(|args| match args.command.as_str() {
        "build" => build(&args),
        _ => inspect(&args),
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure(code, why)) => {
            eprintln!("kel-native: {why}");
            ExitCode::from(code)
        }
    }
}
