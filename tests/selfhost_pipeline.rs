// The self-hosted compiler is a full-width host tool. Its byte-level and
// op-encoding arithmetic overflows a narrow declared word, so these tests are
// meaningful only on a 64-bit runtime, not under the `narrow-word-*` configs.
#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]
//! End-to-end lexer-into-parser pipeline, in three layers:
//!
//! 1. Boundary equivalence: for real stage source the self-hosted lexer
//!    (`kel/lexer.kel`, increment 5) produces exactly the token stream the parser
//!    harness's `adapt_tokens` adapter produces from the runtime tokenizer, token
//!    for token and interned id for interned id. That adapter is the reference
//!    parse.kel is validated against, so this establishes the lexer as a verified
//!    drop-in for it.
//! 2. Metadata recovery: the host recovers parse.kel's non-token inputs -- the
//!    interned ids of `limit` and `require`, and the chunk table of function-name
//!    ids -- from the lexer's output alone, via the intern table the lexer exposes
//!    in shared data plus a brace-depth scan of the token stream.
//! 3. Actual composition: lexer.kel tokenizes the source, the host builds
//!    parse.kel's inputs from that output, and parse.kel parses it, with the host
//!    only orchestrating the yield/resume loops and no runtime-tokenizer adapter
//!    anywhere in the path. The parse shape matches the reference.

use keleusma::Arena;
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::TokenKind;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

/// The reference token stream: the runtime tokenizer mapped exactly as the parser
/// harness's `adapt_tokens` maps it, to `(Tok code, payload)` pairs with
/// identifiers interned in first-seen order. This is a verbatim copy of that
/// adapter's mapping (kept in sync by construction), minus the trailing EOF, which
/// the adapter drops with `continue`.
fn reference_stream(src: &str) -> Vec<(i64, i64)> {
    reference_stream_and_names(src).0
}

/// As [`reference_stream`], also returning the first-seen interned-name table the
/// adapter builds, against which the lexer's exposed intern table is checked.
fn reference_stream_and_names(src: &str) -> (Vec<(i64, i64)>, Vec<String>) {
    let mut names: Vec<String> = Vec::new();
    let mut intern = |s: &str| -> i64 {
        if let Some(i) = names.iter().position(|n| n == s) {
            i as i64
        } else {
            names.push(s.to_string());
            (names.len() - 1) as i64
        }
    };
    let tokens = tokenize(src).expect("lex");
    let mut out = Vec::new();
    for tok in &tokens {
        let (kind, val) = match &tok.kind {
            TokenKind::Fn => (0, 0),
            TokenKind::LowerIdent(s) | TokenKind::UpperIdent(s) => (1, intern(s)),
            TokenKind::LBrace => (2, 0),
            TokenKind::RBrace => (3, 0),
            TokenKind::Yield => (5, 0),
            TokenKind::Loop => (6, 0),
            TokenKind::LParen => (7, 0),
            TokenKind::RParen => (8, 0),
            TokenKind::Colon => (9, 0),
            TokenKind::Comma => (10, 0),
            TokenKind::IntLit(n) => (12, *n),
            TokenKind::Data => (13, 0),
            TokenKind::Shared => (14, 0),
            TokenKind::Private => (15, 0),
            TokenKind::Const => (16, 0),
            TokenKind::Eq => (17, 0),
            TokenKind::Use => (19, 0),
            TokenKind::LBracket => (41, 0),
            TokenKind::RBracket => (42, 0),
            TokenKind::Plus => (21, 0),
            TokenKind::Minus => (22, 0),
            TokenKind::Star => (23, 0),
            TokenKind::Slash => (24, 0),
            TokenKind::Percent => (25, 0),
            TokenKind::EqEq => (26, 0),
            TokenKind::NotEq => (27, 0),
            TokenKind::Lt => (28, 0),
            TokenKind::Gt => (29, 0),
            TokenKind::LtEq => (30, 0),
            TokenKind::GtEq => (31, 0),
            TokenKind::Not => (32, 0),
            TokenKind::Band => (33, 0),
            TokenKind::Bor => (34, 0),
            TokenKind::Bxor => (35, 0),
            TokenKind::Andalso => (36, 0),
            TokenKind::Orelse => (37, 0),
            TokenKind::Let => (38, 0),
            TokenKind::Semicolon => (39, 0),
            TokenKind::Dot => (40, 0),
            TokenKind::If => (43, 0),
            TokenKind::Else => (44, 0),
            TokenKind::For => (45, 0),
            TokenKind::In => (46, 0),
            TokenKind::DotDot => (47, 0),
            TokenKind::Match => (48, 0),
            TokenKind::FatArrow => (49, 0),
            TokenKind::Underscore => (50, 0),
            TokenKind::ColonColon => (51, 0),
            TokenKind::As => (52, 0),
            TokenKind::Enum => (53, 0),
            TokenKind::Eof => continue,
            _ => (4, 0),
        };
        out.push((kind, val));
    }
    (out, names)
}

// Flat shared-data slot indices of the lexer's intern table, in the `src` block's
// declaration order: len (1 slot) then bytes (245760) precede it.
const LEX_ISTART: usize = 1 + 245760;
const LEX_ILEN: usize = 1 + 245760 + 1280;
const LEX_ICOUNT: usize = 1 + 245760 + 1280 + 1280;

fn shared_word(vm: &Vm, buf: &[u8], slot: usize) -> i64 {
    match vm.get_shared(buf, slot).expect("get_shared") {
        Value::Int(n) => n,
        other => panic!("expected Int at slot {slot}, got {other:?}"),
    }
}

/// Drive `kel/lexer.kel` over `src`, returning its `(tok, payload)` stream (without
/// the trailing EOF) and the id-to-spelling table recovered from the exposed
/// intern table: for each interned id its `(istart, ilen)` locates the name's
/// bytes in the source, exactly the map the parser's `limit`/`require`/chunk-name
/// inputs are expressed in, obtained from the lexer's output with no re-lexing.
fn lex_full(src: &str) -> (Vec<(i64, i64)>, Vec<String>) {
    let bytes = src.as_bytes();
    let source = std::fs::read_to_string("compiler/kel/lexer.kel").expect("read lexer.kel");
    let m = compile(&parse(&tokenize(&source).expect("lex")).expect("parse")).expect("compile");
    let need = required_persistent_capacity_for(&m);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(m, &arena).expect("verify");

    assert!(
        bytes.len() <= 245760,
        "source exceeds the lexer's 245760-byte cap"
    );
    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, 0, Value::Int(bytes.len() as i64))
        .expect("len");
    for (i, &b) in bytes.iter().enumerate() {
        vm.set_shared(&mut shared, 1 + i, Value::Byte(b))
            .expect("byte");
    }

    let mut out = Vec::new();
    let mut st = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    let mut reached_eof = false;
    for _ in 0..(bytes.len() * 4 + 16) {
        if let VmState::Yielded(Value::Int(t)) = st {
            if t == 63 {
                // PENDING; skip.
            } else if t == 62 {
                reached_eof = true;
                break;
            } else {
                out.push((t % 64, t / 64));
            }
        } else if !matches!(st, VmState::Reset) {
            panic!("unexpected {st:?}");
        }
        st = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    assert!(reached_eof, "lexer did not reach EOF within the budget");

    // Recover the id-to-spelling table from the exposed intern table.
    let icount = shared_word(&vm, &shared, LEX_ICOUNT) as usize;
    let mut names = Vec::with_capacity(icount);
    for id in 0..icount {
        let start = shared_word(&vm, &shared, LEX_ISTART + id) as usize;
        let len = shared_word(&vm, &shared, LEX_ILEN + id) as usize;
        names.push(String::from_utf8(bytes[start..start + len].to_vec()).expect("utf8"));
    }
    (out, names)
}

fn lex_stream(src: &str) -> Vec<(i64, i64)> {
    lex_full(src).0
}

/// The chunk table the parser needs: the function-name ids in declaration order,
/// recovered by scanning the token stream for a category keyword (fn 0, yield 5,
/// loop 6) at brace depth 0 immediately followed by an Ident. Brace-depth tracking
/// distinguishes a top-level `yield` head from a `yield` expression inside a body.
fn chunk_ids_from_tokens(tokens: &[(i64, i64)]) -> Vec<i64> {
    let mut chunks = Vec::new();
    let mut depth = 0i64;
    for w in tokens.windows(2) {
        let (tok, _) = w[0];
        let (next_tok, next_val) = w[1];
        match tok {
            2 => depth += 1, // {
            3 => depth -= 1, // }
            0 | 5 | 6 if depth == 0 && next_tok == 1 => chunks.push(next_val),
            _ => {}
        }
    }
    chunks
}

/// Assert the self-hosted lexer reproduces the reference adapter's stream exactly.
fn assert_lexer_matches_adapter(src: &str) {
    assert_eq!(lex_stream(src), reference_stream(src), "source: {src:?}");
}

#[test]
fn lexer_matches_the_adapter_on_a_simple_function() {
    assert_lexer_matches_adapter("fn f(x: Word) -> Word { x + 1 }");
}

#[test]
fn lexer_matches_the_adapter_on_identifiers_with_underscores_and_digits() {
    // The identifier-boundary fixes must agree with the runtime tokenizer on
    // underscores (mid-run and leading) and trailing digits.
    assert_lexer_matches_adapter(
        "fn is_ident_cont(b: Word) -> Word { \
            if is_alpha(b) == 1 { 1 } else { if is_digit(b) == 1 { 1 } else { 0 } } }",
    );
}

#[test]
fn lexer_matches_the_adapter_on_a_when_guarded_yield_head() {
    // `when` is a reserved word the stages use but the parser has no Tok for, so
    // both the adapter and the lexer fold it to the catch-all 4 rather than
    // interning it as an identifier; this checks that alignment.
    assert_lexer_matches_adapter("yield emit(resume: Word) -> Word when pos < len { yield buf }");
}

#[test]
fn lexer_matches_the_adapter_on_the_operator_and_punctuation_surface() {
    // Every compound and single-byte operator, the brackets, the arrow, and a
    // match arm with `=>` and `_`, so the whole punctuation surface is exercised.
    assert_lexer_matches_adapter(
        "fn g(a: Word) -> Word { \
            let r = a * 2 + 1 - 3 / 1 % 4; \
            if r >= 2 andalso r <= 9 orelse r != 0 { \
                match r { 0 => a, _ => r band 1 bor 2 bxor 3 } \
            } else { a[r] } }",
    );
}

#[test]
fn lexer_matches_the_adapter_on_a_data_block_and_enum() {
    assert_lexer_matches_adapter(
        "enum Kind { Lo = 0, Hi = 1 } \
         shared data src { buf: [Word; 16], len: Word } \
         private data st { pos: Word } \
         fn scan(t: Word) -> Word { \
            for i in 0..src.len limit 16 { if src.buf[i] == t { st.pos = i; } } st.pos }",
    );
}

#[test]
fn lexer_matches_the_adapter_on_a_verbatim_stage_function() {
    // A real function copied from lexer.kel itself: keyword classification by run
    // length, dispatching over `peek_at` calls, with underscores, digits, and the
    // `0 - 1` sentinel. The lexer tokenizing its own source into the parser's
    // vocabulary, identically to the reference adapter.
    assert_lexer_matches_adapter(
        "fn keyword_code(start: Word, len: Word) -> Word { \
            if len == 2 { kw2(start) } else { \
            if len == 3 { kw3(start) } else { \
            if len == 4 { kw4(start) } else { \
            0 - 1 } } } }",
    );
}

// The host recovers the parser stage's non-token inputs -- the interned id of
// `limit` and `require` (contextual identifiers the parser matches by id) and the
// chunk table (function-name ids in declaration order) -- from lexer.kel's output
// alone: the exposed intern table plus a brace-depth scan of the token stream.
// This is the enabling step for composing lexer.kel into parse.kel without the
// runtime-tokenizer adapter, so it is checked against that adapter's own values.
#[test]
fn host_recovers_parser_metadata_from_lexer_output() {
    let src = "require word >= 32; \
        shared data src { buf: [Word; 8], len: Word } \
        fn clamp(x: Word) -> Word { if x > 8 { 8 } else { x } } \
        yield emit(resume: Word) -> Word when src.len > 0 { yield src.buf[0] } \
        fn scan(t: Word) -> Word { for i in 0..src.len limit 8 { } t }";

    let (tokens, names) = lex_full(src);
    let (ref_tokens, ref_names) = reference_stream_and_names(src);

    // The lexer's stream and its intern order both match the reference adapter.
    assert_eq!(tokens, ref_tokens, "token stream");
    assert_eq!(names, ref_names, "intern order");

    // limit_id and require_id: the ids of those contextual identifiers, recovered
    // from the lexer's own intern table, match the adapter's first-seen positions.
    let id_of = |s: &str| names.iter().position(|n| n == s).map(|i| i as i64);
    assert_eq!(
        id_of("limit"),
        ref_names
            .iter()
            .position(|n| n == "limit")
            .map(|i| i as i64)
    );
    assert_eq!(
        id_of("require"),
        ref_names
            .iter()
            .position(|n| n == "require")
            .map(|i| i as i64)
    );
    assert!(id_of("limit").is_some() && id_of("require").is_some());

    // The chunk table recovered by scanning the tokens equals the parser's own
    // function list (clamp, emit, scan) interned to ids.
    let chunks = chunk_ids_from_tokens(&tokens);
    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    let ref_chunks: Vec<i64> = program
        .functions
        .iter()
        .map(|f| {
            ref_names
                .iter()
                .position(|n| n == &f.name)
                .expect("name interned") as i64
        })
        .collect();
    assert_eq!(chunks, ref_chunks, "chunk table");
    assert_eq!(chunks.len(), 3); // clamp, emit, scan
}

// Flat shared-data slot indices of parse.kel's `toks` block, matching the parser
// harness: len, packed[40960] (one `tok+payload*64` word per token), limit_id,
// chunk_count, chunks[256], require_id, in declaration order.
const P_LEN: usize = 0;
const P_PACKED: usize = 1;
const P_LIMIT_ID: usize = 1 + 40960;
const P_CHUNK_COUNT: usize = 1 + 40960 + 1;
const P_CHUNKS: usize = 1 + 40960 + 2;
const P_REQUIRE_ID: usize = 1 + 40960 + 2 + 256;
const P_WORD_ID: usize = 1 + 40960 + 2 + 256 + 1;
const P_BYTE_ID: usize = 1 + 40960 + 2 + 256 + 2;
const P_BOOL_ID: usize = 1 + 40960 + 2 + 256 + 3;

/// Compile parse.kel on a 64MB thread; its deeply nested source overflows the
/// default 2MB test-thread stack in the host compiler's recursive-descent parse.
fn compile_parse_stage() -> keleusma::bytecode::Module {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let stage = std::fs::read_to_string("compiler/kel/parse.kel").expect("read parse.kel");
            compile(&parse(&tokenize(&stage).expect("lex parse.kel")).expect("parse parse.kel"))
                .expect("compile parse.kel")
        })
        .expect("spawn")
        .join()
        .expect("join")
}

/// The actual lexer-into-parser pipeline: lex `src` with lexer.kel, recover the
/// parser's non-token inputs from the lexer's output, drive parse.kel with them,
/// and decode the record stream. Returns (function count, total body-node count).
/// The host only tokenizes with lexer.kel and orchestrates the yield/resume loops;
/// no runtime-tokenizer adapter is used.
fn parse_via_lexer(src: &str) -> (usize, usize) {
    let (tokens, names) = lex_full(src);
    let id_of = |s: &str| {
        names
            .iter()
            .position(|n| n == s)
            .map(|i| i as i64)
            .unwrap_or(-1)
    };
    let limit_id = id_of("limit");
    let require_id = id_of("require");
    let word_id = id_of("Word");
    let byte_id = id_of("Byte");
    let bool_id = id_of("Bool");
    let chunks = chunk_ids_from_tokens(&tokens);

    let module = compile_parse_stage();
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verify parse.kel");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, P_LEN, Value::Int(tokens.len() as i64))
        .unwrap();
    vm.set_shared(&mut shared, P_LIMIT_ID, Value::Int(limit_id))
        .unwrap();
    vm.set_shared(&mut shared, P_REQUIRE_ID, Value::Int(require_id))
        .unwrap();
    vm.set_shared(&mut shared, P_WORD_ID, Value::Int(word_id))
        .unwrap();
    vm.set_shared(&mut shared, P_BYTE_ID, Value::Int(byte_id))
        .unwrap();
    vm.set_shared(&mut shared, P_BOOL_ID, Value::Int(bool_id))
        .unwrap();
    vm.set_shared(&mut shared, P_CHUNK_COUNT, Value::Int(chunks.len() as i64))
        .unwrap();
    for (i, &c) in chunks.iter().enumerate() {
        vm.set_shared(&mut shared, P_CHUNKS + i, Value::Int(c))
            .unwrap();
    }
    for (i, &(k, v)) in tokens.iter().enumerate() {
        vm.set_shared(&mut shared, P_PACKED + i, Value::Int(k + v * 64))
            .unwrap();
    }

    // Decode the record stream exactly as the parser harness does, but keep only
    // the counts: how many function declarations and how many body nodes.
    let mut funcs = 0usize;
    let mut body_nodes = 0usize;
    let (mut in_body, mut in_data, mut in_enum, mut in_use) = (false, false, false, false);
    let mut in_guard = false;
    let mut open_decl = false;
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    for _ in 0..(tokens.len() * 4 + 64) {
        if let VmState::Yielded(Value::Int(w)) = state {
            let code = w.rem_euclid(64);
            if in_body {
                match code {
                    0 => {}
                    15 => in_body = false,
                    _ => body_nodes += 1,
                }
            } else if in_guard {
                if code == 15 {
                    in_guard = false; // the guard forest's Done
                }
            } else if in_data {
                if code == 5 {
                    in_data = false;
                }
            } else if in_enum {
                if code == 5 {
                    in_enum = false;
                }
            } else if in_use {
                if code == 5 {
                    in_use = false;
                }
            } else {
                match code {
                    0 => {}
                    1..=3 => open_decl = true,
                    4 | 6 | 7 | 8 => {}
                    9 => in_data = true,
                    10 => in_use = true,
                    12 => in_enum = true,
                    16 => in_body = true,
                    17 => in_guard = true, // GSTART: a `when` guard forest, skipped here
                    5 => {
                        assert!(open_decl, "END before START");
                        open_decl = false;
                        funcs += 1;
                    }
                    15 => {
                        assert!(!open_decl, "DONE mid-declaration");
                        return (funcs, body_nodes);
                    }
                    other => panic!("unexpected declaration kind {other}"),
                }
            }
        }
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    panic!("parse.kel did not reach DONE within the iteration budget");
}

// The actual end-to-end pipeline: lexer.kel tokenizes real source, the host builds
// parse.kel's inputs from that output alone, and parse.kel parses it. The result
// must match the reference parse's shape (function count and a nonzero body-node
// count), proving the two self-hosted stages compose with the host only
// orchestrating -- no runtime-tokenizer adapter anywhere in the path.
#[test]
fn lexer_into_parser_pipeline_parses_a_multi_declaration_program() {
    let src = "require word >= 32; \
        shared data src { buf: [Word; 8], len: Word } \
        fn clamp(x: Word) -> Word { if x > 8 { 8 } else { x } } \
        yield emit(resume: Word) -> Word when src.len > 0 { yield src.buf[0] } \
        fn scan(t: Word) -> Word { for i in 0..src.len limit 8 { } t }";

    let (funcs, body_nodes) = parse_via_lexer(src);

    let program = parse(&tokenize(src).expect("lex")).expect("parse");
    assert_eq!(funcs, program.functions.len(), "function count");
    assert_eq!(funcs, 3); // clamp, emit, scan
    assert!(body_nodes > 0, "parse.kel produced a body forest");
}

// Comment skipping: `//` line comments must be dropped exactly as the runtime
// tokenizer drops them, so the lexer's stream over commented source equals the
// adapter's over the same source. Real stage files are comment-dense, so this is a
// prerequisite for self-compiling one.
#[test]
fn lexer_skips_line_comments_like_the_adapter() {
    assert_lexer_matches_adapter(
        "fn f(x: Word) -> Word { // a leading comment\n \
            let y = x + 1; // trailing comment\n \
            y // just an identifier then a comment\n }",
    );
    // A comment with punctuation and keywords inside must not tokenize.
    assert_lexer_matches_adapter(
        "// fn not_real(a: Word) { yield a } == != <= :: \n \
         fn g(a: Word) -> Word { a }",
    );
    // A comment running to end of input with no trailing newline.
    assert_lexer_matches_adapter("fn h(a: Word) -> Word { a } // final comment no newline");
}
