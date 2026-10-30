//! **OPERATOR QUESTION 19, STEP 2: A NATIVE STAGE EXECUTOR.**
//!
//! `keleusma_native::stage::NativeStage` runs a self-hosted compiler stage as
//! native code behind one operation, a turn that delivers a reply and returns
//! the next yielded word. The driver's executor boundary is V0.2.X's step 1 and
//! does not exist yet, so the plan's ordering has this file stand in for it. A
//! local trait carries the operations the driver uses, the virtual machine is
//! adapted to it the way the driver drives a stage, resuming again under the
//! same reply after a `Reset`, and ONE host function drives the lexer into the
//! parser through either executor. The two runs must agree in every yield and
//! in both final shared segments.
//!
//! # What this does not establish
//!
//! The driver itself is not run natively, since it calls `Vm` directly. A host
//! native is bound as a C function, and a stage-shaped program calling one is
//! driven against the virtual machine's closure. The ten natives of
//! `frame.kel` are private closures in root `src/crypto_frame/mod.rs`, so that
//! stage is built only to show it is refused unbound. Only the lexer and the
//! parser are driven as real stages. The other stages are built, which shows
//! they lower and compile, and are driven by `stage_differential.rs` through
//! its own harness.
//!
//! Neither the lexer nor the parser reads its reply, which is a tick, so a
//! reply delivered wrongly passes their test, and that was checked by
//! mutation. The native-binding test reads its reply and fails on the same
//! mutation. The lexer and parser test checks the turn rule, one native call
//! per turn against a virtual machine resumed again after a `Reset`, and two
//! native calls per turn fail it, also checked by mutation.
use keleusma::bytecode::{Module, SlotVisibility, Value};
use keleusma::vm::{Vm, VmState, auto_arena_capacity_for, required_persistent_capacity_for};
use keleusma::{compiler::compile, lexer::tokenize, parser::parse};
use keleusma_native::stage::{HostNative, NativeStage};

/// Real source spanning several token classes, as in `stage_differential.rs`.
const SOURCE: &str = "\
// a comment, which the scanner must skip
fn add(a: Word, b: Word) -> Word {
  let c = a + b;
  if c >= 10 { c } else { c * 2 }
}
";

/// More turns than the source has bytes, since the lexer takes one per turn.
const LEX_TURNS: i64 = 400;
const PARSE_TURNS: i64 = 400;
/// The lexer's documented markers, no token yet and end of source.
const PENDING: i64 = 63;
const END: i64 = 62;

/// What the driver asks of a stage. Step 1 would put a trait of this shape in
/// the driver.
trait Executor {
    fn shared_mut(&mut self) -> &mut [u8];
    fn shared(&self) -> &[u8];
    fn turn(&mut self, reply: i64) -> i64;
}

/// The virtual machine, driven as the driver drives it.
struct VmStage<'a> {
    vm: Vm<'a, 'a>,
    shared: Vec<u8>,
    started: bool,
}

impl<'a> VmStage<'a> {
    fn new(module: &Module, arena: &'a keleusma_arena::Arena) -> Self {
        let shared = vec![0u8; keleusma::vm::shared_data_bytes_for(module)];
        let vm = Vm::new(module.clone(), arena).expect("vm loads");
        Self {
            vm,
            shared,
            started: false,
        }
    }
}

fn word(state: VmState) -> Option<i64> {
    match state {
        VmState::Yielded(Value::Int(w)) => Some(w),
        VmState::Reset => None,
        other => panic!("unexpected virtual machine state {other:?}"),
    }
}

impl Executor for VmStage<'_> {
    fn shared_mut(&mut self) -> &mut [u8] {
        &mut self.shared
    }
    fn shared(&self) -> &[u8] {
        &self.shared
    }
    fn turn(&mut self, reply: i64) -> i64 {
        let first = if self.started {
            self.vm
                .resume_with_shared(&mut self.shared, Value::Int(reply))
        } else {
            self.started = true;
            self.vm
                .call_with_shared(&mut self.shared, &[Value::Int(reply)])
        };
        if let Some(w) = word(first.expect("vm turn")) {
            return w;
        }
        let again = self
            .vm
            .resume_with_shared(&mut self.shared, Value::Int(reply))
            .expect("vm resume after reset");
        word(again).expect("a stage yields after one reset")
    }
}

impl Executor for NativeStage {
    fn shared_mut(&mut self) -> &mut [u8] {
        NativeStage::shared_mut(self)
    }
    fn shared(&self) -> &[u8] {
        NativeStage::shared(self)
    }
    fn turn(&mut self, reply: i64) -> i64 {
        NativeStage::turn(self, reply).expect("native turn")
    }
}

fn module_of(path: &str) -> Module {
    let src = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path}: {e}"));
    compile(&parse(&tokenize(&src).expect("lex")).expect("parse")).expect("compile")
}

fn arena_for(m: &Module) -> keleusma_arena::Arena {
    const HOST_MARGIN: usize = 4 << 20;
    let need = required_persistent_capacity_for(m);
    let cap = auto_arena_capacity_for(m, &[]).expect("arena capacity") + need + HOST_MARGIN;
    let mut arena = keleusma_arena::Arena::with_capacity(cap);
    arena.resize_persistent(need).expect("persistent fits");
    arena
}

/// Byte offset of the shared slot named `suffix`, or of its element zero.
fn shared_offset(m: &Module, suffix: &str) -> usize {
    let dl = m.data_layout.as_ref().expect("a data layout");
    let scalar = format!(".{suffix}");
    let element0 = format!(".{suffix}[0]");
    let shared = dl
        .slots
        .iter()
        .filter(|s| s.visibility == SlotVisibility::Shared);
    for (index, s) in shared.enumerate() {
        if s.name == suffix || s.name.ends_with(&scalar) || s.name.ends_with(&element0) {
            return dl.shared_layout[index].offset as usize;
        }
    }
    panic!("no shared slot `{suffix}`")
}

/// Write a length word and an array of `width`-byte elements.
fn seed(m: &Module, buf: &mut [u8], len: &str, array: &str, body: &[i64], width: usize) {
    let at = shared_offset(m, len);
    buf[at..at + 8].copy_from_slice(&(body.len() as u64).to_le_bytes());
    let base = shared_offset(m, array);
    assert!(
        base + body.len() * width <= buf.len(),
        "seed exceeds `{array}`"
    );
    for (i, v) in body.iter().enumerate() {
        let at = base + i * width;
        match width {
            1 => buf[at] = *v as u8,
            8 => buf[at..at + 8].copy_from_slice(&v.to_le_bytes()),
            other => panic!("unsupported element width {other}"),
        }
    }
}

struct Run {
    lexed: Vec<i64>,
    tokens: Vec<i64>,
    parsed: Vec<i64>,
    lex_shared: Vec<u8>,
    parse_shared: Vec<u8>,
}

/// The one host. It seeds the lexer, turns it to end of source, hands the
/// tokens to the parser by seeding its shared segment, and turns the parser.
fn lex_then_parse(
    lex_module: &Module,
    lexer: &mut impl Executor,
    parse_module: &Module,
    parser: &mut impl Executor,
) -> Run {
    let bytes: Vec<i64> = SOURCE.bytes().map(i64::from).collect();
    seed(lex_module, lexer.shared_mut(), "len", "bytes", &bytes, 1);
    let mut lexed = Vec::new();
    let mut tokens = Vec::new();
    for t in 0..LEX_TURNS {
        let w = lexer.turn(t);
        lexed.push(w);
        match w & 0xFF {
            PENDING => {}
            END => break,
            _ => tokens.push(w),
        }
    }
    seed(
        parse_module,
        parser.shared_mut(),
        "len",
        "packed",
        &tokens,
        8,
    );
    let parsed = (0..PARSE_TURNS).map(|t| parser.turn(t)).collect();
    Run {
        lexed,
        tokens,
        parsed,
        lex_shared: lexer.shared().to_vec(),
        parse_shared: parser.shared().to_vec(),
    }
}

#[test]
fn the_lexer_and_parser_agree_through_the_native_executor() {
    let lex = module_of("../src/selfhost/kel/lexer.kel");
    let par = module_of("../src/selfhost/kel/parse.kel");
    let (lex_arena, par_arena) = (arena_for(&lex), arena_for(&par));
    let vm = lex_then_parse(
        &lex,
        &mut VmStage::new(&lex, &lex_arena),
        &par,
        &mut VmStage::new(&par, &par_arena),
    );
    let native = lex_then_parse(
        &lex,
        &mut NativeStage::new(&lex).expect("lexer stage"),
        &par,
        &mut NativeStage::new(&par).expect("parser stage"),
    );

    // Vacuity guards. A lexer that never reached end of source, or a parser
    // that yields only its unseeded pair, would agree for no reason.
    assert_eq!(
        vm.lexed.last().map(|w| w & 0xFF),
        Some(END),
        "no end of source"
    );
    assert!(vm.tokens.len() > 15, "only {} tokens", vm.tokens.len());
    let mut distinct = vm.parsed.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert!(distinct.len() >= 3, "the parser yielded only {distinct:?}");

    assert_eq!(vm.lexed, native.lexed, "lexer yields");
    assert_eq!(vm.parsed, native.parsed, "parser yields");
    assert!(vm.lex_shared == native.lex_shared, "lexer shared segments");
    assert!(
        vm.parse_shared == native.parse_shared,
        "parser shared segments"
    );
    eprintln!(
        "  {} lexer turns, {} tokens and {} parser turns agree",
        vm.lexed.len(),
        vm.tokens.len(),
        vm.parsed.len()
    );
}

/// Each stage source builds as a native stage, and `frame.kel`, whose host
/// natives are not bound here, is refused by name. The build time is
/// reported, since the plan defers an ahead-of-time archive until the
/// in-process cost is measured.
#[test]
fn every_stage_builds_and_an_unbound_host_native_is_refused() {
    let dir = std::path::Path::new("../src/selfhost/kel");
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .expect("stage directory")
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "kel"))
        .collect();
    paths.sort();
    // Fifteen since absorption 119 added `template_scan.kel`.
    assert_eq!(paths.len(), 15, "the driver embeds fifteen stages");
    for p in &paths {
        let m = module_of(p.to_str().unwrap());
        let start = std::time::Instant::now();
        let stage = NativeStage::new(&m).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        eprintln!(
            "  {}: {} shared bytes, built in {} ms",
            p.file_name().unwrap().to_string_lossy(),
            stage.shared().len(),
            start.elapsed().as_millis()
        );
    }
    let frame = module_of("../src/crypto_frame/frame.kel");
    let refusal = NativeStage::new(&frame)
        .err()
        .expect("frame.kel is refused");
    assert!(refusal.to_string().contains("is not bound"), "{refusal}");
}

/// A stage-shaped program that calls one two-argument host native and keeps
/// a running total in its shared segment, so a native result that is lost,
/// misrouted or bound to the wrong function changes both the yields and the
/// segment.
const WITH_A_NATIVE: &str = "use host::scale(Word, Word) -> Word
shared data io { total: Word }
loop main(a: Word) -> Word {
    let r = host::scale(a, 3);
    io.total = io.total + r;
    yield io.total
}";

/// The native both sides bind, chosen to be neither the identity nor
/// symmetric in its arguments.
fn scale(a: i64, b: i64) -> i64 {
    a.wrapping_mul(b).wrapping_add(7)
}

unsafe extern "C" fn scale_stub(a: i64, b: i64) -> i64 {
    scale(a, b)
}

unsafe extern "C" fn one_word_stub(a: i64) -> i64 {
    a
}

#[test]
fn a_bound_host_native_agrees_with_the_vm_closure() {
    let m = keleusma_native_test_module(WITH_A_NATIVE);
    let arena = arena_for(&m);
    let mut vm = VmStage::new(&m, &arena);
    vm.vm
        .register_native_closure("host::scale", |args: &[Value]| {
            let (Value::Int(a), Value::Int(b)) = (&args[0], &args[1]) else {
                panic!("scale takes two words");
            };
            Ok(Value::Int(scale(*a, *b)))
        });
    let mut native =
        NativeStage::with_natives(&m, &[("host::scale", HostNative::Arity2(scale_stub))])
            .expect("a stage with one bound native");
    let replies: Vec<i64> = (0..40).map(|i| (i * 13) % 17 - 5).collect();
    let on_vm: Vec<i64> = replies.iter().map(|r| vm.turn(*r)).collect();
    let on_native: Vec<i64> = replies
        .iter()
        .map(|r| Executor::turn(&mut native, *r))
        .collect();
    let mut distinct = on_vm.clone();
    distinct.sort_unstable();
    distinct.dedup();
    assert!(distinct.len() > 20, "the total barely moved: {distinct:?}");
    assert_eq!(on_vm, on_native, "yields");
    assert!(vm.shared == Executor::shared(&native), "shared segments");
}

#[test]
fn a_host_native_binding_must_match_what_the_stage_calls() {
    let m = keleusma_native_test_module(WITH_A_NATIVE);
    let refusal = |natives: &[(&str, HostNative)]| {
        NativeStage::with_natives(&m, natives)
            .err()
            .expect("refused")
            .to_string()
    };
    let unbound = refusal(&[]);
    assert!(unbound.contains("is not bound"), "{unbound}");
    let arity = refusal(&[("host::scale", HostNative::Arity1(one_word_stub))]);
    assert!(
        arity.contains("called with 2 arguments and bound with 1"),
        "{arity}"
    );
    let extra = refusal(&[
        ("host::scale", HostNative::Arity2(scale_stub)),
        ("host::scael", HostNative::Arity2(scale_stub)),
    ]);
    assert!(
        extra.contains("`host::scael`, which the stage does not call"),
        "{extra}"
    );
}

fn keleusma_native_test_module(src: &str) -> Module {
    compile(&parse(&tokenize(src).expect("lex")).expect("parse")).expect("compile")
}
