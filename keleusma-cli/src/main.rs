//! Standalone command-line frontend for Keleusma.
//!
//! Provides three subcommands modeled after Rhai's CLI tooling:
//!
//! - `run <file>` parses, compiles, verifies, and executes a Keleusma
//!   script. Pre-registers utility and math natives.
//! - `compile <file> [-o <output>]` produces a serialized bytecode
//!   file that hosts can load through `Vm::load_bytes`.
//! - `repl` starts an interactive prompt where expressions and
//!   declarations accumulate into a session prefix.
//!
//! As a shorthand, any first argument ending in `.kel` is treated as
//! a `run` invocation, so `keleusma hello.kel` runs the script.

use std::env;
use std::fs;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::ExitCode;

use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::utility_natives::register_utility_natives;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};
use keleusma::{Arena, Value};

const REPL_BANNER: &str = "Keleusma REPL. Type :help for commands, :quit to exit.";

const REPL_RETURN_TYPES: &[&str] = &["i64", "f64", "bool", "String", "()"];

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let subcommand = match args.get(1) {
        Some(s) => s.as_str(),
        None => {
            print_help();
            return ExitCode::SUCCESS;
        }
    };

    match subcommand {
        "run" => run_subcommand(&args[2..]),
        "compile" => compile_subcommand(&args[2..]),
        "repl" => repl_subcommand(&args[2..]),
        "--help" | "-h" | "help" => {
            print_help();
            ExitCode::SUCCESS
        }
        "--version" | "-V" | "version" => {
            println!("keleusma {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        other => {
            // Treat any remaining argument as a script path. This
            // covers both `keleusma file.kel` (extension shorthand)
            // and `keleusma /path/to/extensionless-shebang-script`
            // when the kernel invokes us through a `#!/usr/bin/env
            // keleusma` line. If the file is missing, the error
            // surfaces in `run_file`.
            if std::path::Path::new(other).is_file() {
                run_file(other)
            } else {
                eprintln!("error: unknown subcommand or missing file `{}`", other);
                print_help();
                ExitCode::FAILURE
            }
        }
    }
}

fn print_help() {
    println!("keleusma: command-line frontend for the Keleusma scripting language");
    println!();
    println!("Usage:");
    println!("  keleusma <subcommand> [options]");
    println!("  keleusma <file>.kel               (shorthand for `run`)");
    println!();
    println!("Subcommands:");
    println!("  run <file>                        Compile and execute a script");
    println!("  compile <file> [-o <output>]      Compile to bytecode");
    println!("  repl                              Start interactive REPL");
    println!("  help, --help, -h                  Show this help");
    println!("  version, --version, -V            Show version");
    println!();
    println!("Examples:");
    println!("  keleusma run hello.kel");
    println!("  keleusma hello.kel");
    println!("  keleusma compile hello.kel -o hello.kel.bin");
    println!("  keleusma repl");
}

fn run_subcommand(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("error: `run` requires a script path");
        return ExitCode::FAILURE;
    }
    run_file(&args[0])
}

fn run_file(path: &str) -> ExitCode {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: reading {}: {}", path, e);
            return ExitCode::FAILURE;
        }
    };
    // Detect compiled bytecode by magic. The bytecode loader strips
    // a leading shebang line, so check both at offset 0 and after a
    // `#!...\n` envelope.
    if looks_like_bytecode(&bytes) {
        match execute_bytecode(&bytes) {
            Ok(value) => {
                print_value(&value);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {}", e);
                ExitCode::FAILURE
            }
        }
    } else {
        let source = match core::str::from_utf8(&bytes) {
            Ok(s) => s,
            Err(_) => {
                eprintln!(
                    "error: {} is neither valid UTF-8 source nor recognised bytecode",
                    path
                );
                return ExitCode::FAILURE;
            }
        };
        match execute_source(source) {
            Ok(value) => {
                print_value(&value);
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {}", e);
                ExitCode::FAILURE
            }
        }
    }
}

fn looks_like_bytecode(bytes: &[u8]) -> bool {
    let after_shebang = if bytes.starts_with(b"#!") {
        match bytes.iter().position(|&b| b == b'\n') {
            Some(nl) => &bytes[nl + 1..],
            None => return false,
        }
    } else {
        bytes
    };
    after_shebang.starts_with(b"KELE")
}

fn compile_subcommand(args: &[String]) -> ExitCode {
    if args.is_empty() {
        eprintln!("error: `compile` requires a script path");
        return ExitCode::FAILURE;
    }
    let input = &args[0];
    let mut output: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                if i + 1 >= args.len() {
                    eprintln!("error: --output requires a path");
                    return ExitCode::FAILURE;
                }
                output = Some(args[i + 1].clone());
                i += 2;
            }
            other => {
                eprintln!("error: unknown option `{}`", other);
                return ExitCode::FAILURE;
            }
        }
    }
    let output_path = output.unwrap_or_else(|| default_output_path(input));

    let source = match fs::read_to_string(input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: reading {}: {}", input, e);
            return ExitCode::FAILURE;
        }
    };
    let module = match compile_source(&source) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: {}", e);
            return ExitCode::FAILURE;
        }
    };
    let bytes = match module.to_bytes() {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: serializing bytecode: {:?}", e);
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = fs::write(&output_path, &bytes) {
        eprintln!("error: writing {}: {}", output_path, e);
        return ExitCode::FAILURE;
    }
    eprintln!("wrote {} ({} bytes)", output_path, bytes.len());
    ExitCode::SUCCESS
}

fn default_output_path(input: &str) -> String {
    let path = Path::new(input);
    if path.extension().and_then(|s| s.to_str()) == Some("kel") {
        format!("{}.bin", input)
    } else {
        format!("{}.kel.bin", input)
    }
}

fn repl_subcommand(_args: &[String]) -> ExitCode {
    println!("{}", REPL_BANNER);
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut prefix = String::new();
    let mut input = String::new();

    loop {
        {
            let mut out = stdout.lock();
            let _ = out.write_all(b"> ");
            let _ = out.flush();
        }
        input.clear();
        let bytes_read = match stdin.lock().read_line(&mut input) {
            Ok(n) => n,
            Err(e) => {
                eprintln!("error: reading input: {}", e);
                return ExitCode::FAILURE;
            }
        };
        if bytes_read == 0 {
            // EOF (Ctrl-D).
            println!();
            return ExitCode::SUCCESS;
        }
        let line = input.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(stripped) = line.strip_prefix(':') {
            match stripped {
                "quit" | "q" | "exit" => return ExitCode::SUCCESS,
                "help" | "h" => print_repl_help(),
                "reset" => {
                    prefix.clear();
                    println!("session prefix cleared");
                }
                "show" => {
                    if prefix.is_empty() {
                        println!("(empty session prefix)");
                    } else {
                        println!("{}", prefix);
                    }
                }
                other => {
                    eprintln!("error: unknown REPL command `:{}`", other);
                }
            }
            continue;
        }
        evaluate_repl_input(&mut prefix, line);
    }
}

fn print_repl_help() {
    println!("Keleusma REPL commands:");
    println!("  :help, :h               Show this help");
    println!("  :quit, :q, :exit        Exit the REPL");
    println!("  :reset                  Clear the session prefix");
    println!("  :show                   Display the current session prefix");
    println!();
    println!("Otherwise, type:");
    println!("  An expression to evaluate it (`1 + 2`, `double(21)`)");
    println!(
        "  A declaration to add to the session prefix (`fn`, `struct`, `enum`, `trait`, `impl`, `use`)"
    );
}

/// Decide whether a REPL line is a declaration (added to the prefix)
/// or an expression (evaluated against the prefix).
fn is_declaration(line: &str) -> bool {
    let starters = [
        "fn ", "yield ", "loop ", "struct ", "enum ", "trait ", "impl ", "use ", "data ",
    ];
    starters.iter().any(|s| line.starts_with(s))
}

fn evaluate_repl_input(prefix: &mut String, line: &str) {
    if is_declaration(line) {
        // Tentatively append; verify it parses and compiles within
        // the prefix before committing.
        let candidate = format!("{}\n{}", prefix.trim_end(), line);
        let candidate = candidate.trim().to_string();
        // Add a trivial main if the prefix lacks one so compilation
        // can proceed. The trivial main is dropped before commit.
        let probe = if has_main(&candidate) {
            candidate.clone()
        } else {
            format!("{}\n\nfn main() -> i64 {{ 0 }}\n", candidate)
        };
        match compile_source(&probe) {
            Ok(_) => {
                *prefix = candidate;
                if let Some(name) = extract_decl_name(line) {
                    println!("defined: {}", name);
                } else {
                    println!("declaration accepted");
                }
            }
            Err(e) => {
                eprintln!("error: {}", e);
            }
        }
        return;
    }

    // Expression. Try wrapping with each candidate return type until
    // one type-checks. Run the first that compiles.
    for return_type in REPL_RETURN_TYPES {
        let body = if *return_type == "()" {
            format!("{}; ()", line)
        } else {
            line.to_string()
        };
        let program = format!(
            "{}\n\nfn main() -> {} {{ {} }}\n",
            prefix.trim_end(),
            return_type,
            body
        );
        if let Ok(value) = execute_source(&program) {
            print_value(&value);
            return;
        }
    }
    // None of the types worked. Run with i64 to surface the actual
    // error message to the user.
    let program = format!("{}\n\nfn main() -> i64 {{ {} }}\n", prefix.trim_end(), line);
    match execute_source(&program) {
        Ok(value) => print_value(&value),
        Err(e) => eprintln!("error: {}", e),
    }
}

fn has_main(source: &str) -> bool {
    // Very rough heuristic: look for "fn main", "yield main", or
    // "loop main" at the start of any line. Sufficient for REPL
    // pipelining where users either declare a main themselves or
    // expect the REPL to wrap.
    source.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("fn main")
            || trimmed.starts_with("yield main")
            || trimmed.starts_with("loop main")
    })
}

fn extract_decl_name(line: &str) -> Option<String> {
    // For declarations, extract the name following the keyword. Used
    // for the REPL "defined: name" feedback.
    let mut tokens = line.split_whitespace();
    let kw = tokens.next()?;
    let name = match kw {
        "fn" | "yield" | "loop" | "struct" | "enum" | "trait" | "data" => {
            let next = tokens.next()?;
            // The name may have boundary characters (`(`, `<`, `{`,
            // `:`) attached without a space. Split at the first such
            // character and take the prefix.
            let end = next.find(['(', '<', '{', ':']).unwrap_or(next.len());
            next[..end].to_string()
        }
        "impl" => {
            // `impl Trait for Type { ... }` -- show "impl Trait for Type".
            let rest = tokens.collect::<Vec<&str>>().join(" ");
            let head = rest.split('{').next().unwrap_or(&rest).trim().to_string();
            format!("impl {}", head)
        }
        "use" => {
            let next = tokens.next()?;
            format!("use {}", next.trim_end_matches([';', ','].as_ref()))
        }
        _ => return None,
    };
    Some(name)
}

/// Compile a complete source program through the standard pipeline,
/// returning either the resulting `Module` or a stringified error
/// with location information.
fn compile_source(source: &str) -> Result<keleusma::bytecode::Module, String> {
    let tokens = tokenize(source).map_err(|e| format_err("lex", &e.message, e.span))?;
    let program = parse(&tokens).map_err(|e| format_err("parse", &e.message, e.span))?;
    let module = compile(&program).map_err(|e| format_err("compile", &e.message, e.span))?;
    Ok(module)
}

/// Run a source program through compile and execute. The runner
/// pre-registers utility and math natives so scripts can use
/// `to_string`, `length`, `concat`, `slice`, `println`, and the
/// `math::*` family without explicit registration.
fn execute_source(source: &str) -> Result<Value, String> {
    let module = compile_source(source)?;
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::new(module, &arena).map_err(|e| format!("verify: {:?}", e))?;
    drive_to_completion(&mut vm)
}

fn execute_bytecode(bytes: &[u8]) -> Result<Value, String> {
    let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
    let mut vm = Vm::load_bytes(bytes, &arena).map_err(|e| format!("load_bytes: {:?}", e))?;
    drive_to_completion(&mut vm)
}

fn drive_to_completion(vm: &mut Vm) -> Result<Value, String> {
    register_utility_natives(vm);
    match vm.call(&[]).map_err(|e| format!("vm: {:?}", e))? {
        VmState::Finished(v) => Ok(v),
        VmState::Yielded(v) => Err(format!(
            "script yielded but the CLI runner does not yet drive resume: {:?}",
            v
        )),
        VmState::Reset => Err(String::from(
            "script reset but the CLI runner does not yet drive stream cycles",
        )),
    }
}

fn format_err(stage: &str, msg: &str, span: keleusma::token::Span) -> String {
    if span.line == 0 && span.column == 0 {
        format!("{}: {}", stage, msg)
    } else {
        format!("{}: {}:{}: {}", stage, span.line, span.column, msg)
    }
}

fn print_value(v: &Value) {
    match v {
        Value::Int(n) => println!("{}", n),
        Value::Float(f) => println!("{}", f),
        Value::Bool(b) => println!("{}", b),
        Value::StaticStr(s) | Value::DynStr(s) => println!("{}", s),
        Value::Unit => println!("()"),
        Value::None => println!("None"),
        other => println!("{:?}", other),
    }
}
