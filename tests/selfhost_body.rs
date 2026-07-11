//! Body-expression parser (`compiler/kel/body.kel`), increment 3: an expression body
//! over integer literals, parameter references, the binary arithmetic and comparison
//! operators, and parenthesised grouping, lowered by operator precedence to the
//! abstract-syntax node forest the codegen stage consumes.
//!
//! A throwaway adapter tokenises a function, feeds the body's tokens (from the opening
//! `{`) and the function's parameter-name table to the `body.kel` `loop`, and decodes
//! the postorder node-record stream into a node forest. Each leaf record (Literal,
//! Local) pushes a node and its index onto a stack; an interior BinOp record pops its
//! two operands and pushes the combined node. The forest is checked against a
//! reference flattening of the same body's tail expression, with parameters occupying
//! the first frame slots — the same lowering the codegen conformance harness performs
//! — so operator precedence and left-associativity are verified against the reference
//! parser's own tree.

#![cfg(all(
    feature = "compile",
    feature = "verify",
    not(feature = "narrow-word-8"),
    not(feature = "narrow-word-16"),
    not(feature = "narrow-word-32")
))]

use keleusma::Arena;
use keleusma::ast::{BinOp, Expr, Literal, Pattern};
use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::token::TokenKind;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState, required_persistent_capacity_for};

// Shared-data slot offsets, mirroring the `src` block in body.kel: len at 0, the two
// length-512 token arrays, the parameter count, then the length-32 parameter table.
const LEN: usize = 0;
const KINDS: usize = 1;
const VALS: usize = 1 + 512;
const PARAM_COUNT: usize = 1 + 512 + 512;
const PARAMS: usize = 1 + 512 + 512 + 1;

/// One node of the abstract-syntax forest: the codegen contract's `(kind, arg, lhs,
/// rhs)`. A leaf has `lhs == rhs == 0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Node {
    kind: i64,
    arg: i64,
    lhs: i64,
    rhs: i64,
}

/// The parameter name at a slot, for the reference scope.
fn param_name(p: &keleusma::ast::Param) -> &str {
    match &p.pattern {
        Pattern::Variable(n, _) => n,
        other => panic!("test uses simple parameter patterns only, got {other:?}"),
    }
}

/// Drive `body.kel` over the body of the single function in `func_src`, returning the
/// node forest and its root index. Identifier names are interned so the parameter
/// table and the body's identifier tokens share ids.
fn run_body(func_src: &str) -> (Vec<Node>, i64) {
    let program = parse(&tokenize(func_src).expect("lex")).expect("parse");
    let f = &program.functions[0];
    let param_names: Vec<String> = f.params.iter().map(|p| param_name(p).to_string()).collect();

    // Tokenise the whole function, interning identifiers, then keep the tokens from the
    // body's opening `{` (the first `{`, since a signature contains none).
    let mut names: Vec<String> = Vec::new();
    let mut intern = |s: &str| -> i64 {
        if let Some(i) = names.iter().position(|n| n == s) {
            i as i64
        } else {
            names.push(s.to_string());
            (names.len() - 1) as i64
        }
    };
    let tokens = tokenize(func_src).expect("lex");
    let mut kinds = Vec::new();
    let mut vals = Vec::new();
    for tok in &tokens {
        let (kind, val) = match &tok.kind {
            TokenKind::LowerIdent(s) | TokenKind::UpperIdent(s) => (1, intern(s)),
            TokenKind::LBrace => (2, 0),
            TokenKind::RBrace => (3, 0),
            TokenKind::LParen => (7, 0),
            TokenKind::RParen => (8, 0),
            TokenKind::IntLit(n) => (12, *n),
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
            TokenKind::Eof => continue,
            _ => (4, 0),
        };
        kinds.push(kind);
        vals.push(val);
    }
    let body_start = kinds
        .iter()
        .position(|&k| k == 2)
        .expect("a function body opens with `{`");
    let body_kinds = &kinds[body_start..];
    let body_vals = &vals[body_start..];
    let param_ids: Vec<i64> = param_names.iter().map(|n| intern(n)).collect();

    let stage = std::fs::read_to_string("compiler/kel/body.kel").expect("read body.kel");
    let module = compile(&parse(&tokenize(&stage).expect("lex body.kel")).expect("parse body.kel"))
        .expect("compile body.kel");
    let need = required_persistent_capacity_for(&module);
    let mut arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY + need);
    arena.resize_persistent(need).expect("resize");
    let mut vm = Vm::new(module, &arena).expect("verify body.kel");

    let mut shared = vec![0u8; vm.shared_data_bytes()];
    vm.set_shared(&mut shared, LEN, Value::Int(body_kinds.len() as i64))
        .expect("len");
    for (i, (&k, &v)) in body_kinds.iter().zip(body_vals.iter()).enumerate() {
        vm.set_shared(&mut shared, KINDS + i, Value::Int(k))
            .expect("kind");
        vm.set_shared(&mut shared, VALS + i, Value::Int(v))
            .expect("val");
    }
    vm.set_shared(&mut shared, PARAM_COUNT, Value::Int(param_ids.len() as i64))
        .expect("param_count");
    for (i, &id) in param_ids.iter().enumerate() {
        vm.set_shared(&mut shared, PARAMS + i, Value::Int(id))
            .expect("param");
    }

    let mut nodes: Vec<Node> = Vec::new();
    let mut stack: Vec<i64> = Vec::new();
    let mut state = vm
        .call_with_shared(&mut shared, &[Value::Int(0)])
        .expect("call");
    // Each token yields at least once, and an operator spreads its shunting-yard
    // pops and push across extra iterations; with a Reset between every yield, the
    // state budget is a generous multiple of the token count.
    for _ in 0..(body_kinds.len() * 8 + 64) {
        match state {
            VmState::Yielded(Value::Int(w)) => {
                let kind = w.rem_euclid(16);
                let arg = w.div_euclid(16);
                match kind {
                    0 => {} // PENDING
                    1 | 2 => {
                        // A leaf node: Literal (1) or Local (2). Push it and its index.
                        nodes.push(Node {
                            kind,
                            arg,
                            lhs: 0,
                            rhs: 0,
                        });
                        stack.push((nodes.len() - 1) as i64);
                    }
                    3 => {
                        // An interior BinOp node: pop the two operands (rhs then lhs).
                        let rhs = stack.pop().expect("BinOp needs a right operand");
                        let lhs = stack.pop().expect("BinOp needs a left operand");
                        nodes.push(Node {
                            kind,
                            arg,
                            lhs,
                            rhs,
                        });
                        stack.push((nodes.len() - 1) as i64);
                    }
                    15 => {
                        // DONE: the single remaining stack entry is the body root.
                        assert_eq!(stack.len(), 1, "the body has exactly one root node");
                        return (nodes, stack[0]);
                    }
                    other => panic!("unexpected node kind {other}"),
                }
            }
            VmState::Reset => {}
            other => panic!("unexpected VM state {other:?}"),
        }
        state = vm
            .resume_with_shared(&mut shared, Value::Int(0))
            .expect("resume");
    }
    panic!("body parser did not reach DONE within the iteration budget");
}

/// Flatten an expression into `nodes` in postorder, returning the index of its root
/// node. This mirrors the codegen conformance harness's `flatten` for the subset the
/// body parser handles so far: integer literals, parameter references, and the binary
/// arithmetic and comparison operators.
fn flatten(expr: &Expr, scope: &[&str], nodes: &mut Vec<Node>) -> i64 {
    match expr {
        Expr::Literal {
            value: Literal::Int(n),
            ..
        } => {
            nodes.push(Node {
                kind: 1,
                arg: *n,
                lhs: 0,
                rhs: 0,
            });
            (nodes.len() - 1) as i64
        }
        Expr::Ident { name, .. } => {
            let slot = scope
                .iter()
                .position(|n| n == name)
                .expect("identifier is a parameter this increment") as i64;
            nodes.push(Node {
                kind: 2,
                arg: slot,
                lhs: 0,
                rhs: 0,
            });
            (nodes.len() - 1) as i64
        }
        Expr::BinOp {
            op, left, right, ..
        } => {
            let lhs = flatten(left, scope, nodes);
            let rhs = flatten(right, scope, nodes);
            let code = match op {
                BinOp::Add => 1,
                BinOp::Mul => 2,
                BinOp::Sub => 3,
                BinOp::Div => 4,
                BinOp::Mod => 5,
                BinOp::Eq => 6,
                BinOp::NotEq => 7,
                BinOp::Lt => 8,
                BinOp::Gt => 9,
                BinOp::LtEq => 10,
                BinOp::GtEq => 11,
                other => panic!("increment handles arithmetic and comparison, got {other:?}"),
            };
            nodes.push(Node {
                kind: 3,
                arg: code,
                lhs,
                rhs,
            });
            (nodes.len() - 1) as i64
        }
        other => {
            panic!("increment handles literals, parameters, and binary operators, got {other:?}")
        }
    }
}

/// The reference forest for a body: flatten the function's tail expression, with
/// parameters occupying the first frame slots.
fn reference_body(func_src: &str) -> (Vec<Node>, i64) {
    let program = parse(&tokenize(func_src).expect("lex")).expect("parse");
    let f = &program.functions[0];
    let scope: Vec<&str> = f.params.iter().map(param_name).collect();
    let tail = f
        .body
        .tail_expr
        .as_ref()
        .expect("a body is a single tail expression this increment");
    let mut nodes = Vec::new();
    let root = flatten(tail, &scope, &mut nodes);
    (nodes, root)
}

// A body that is a single integer literal is one Literal node.
#[test]
fn a_literal_body_is_one_literal_node() {
    let src = "fn answer() -> Word { 42 }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, root) = run_body(src);
    assert_eq!(root, 0);
    assert_eq!(
        nodes,
        vec![Node {
            kind: 1,
            arg: 42,
            lhs: 0,
            rhs: 0
        }]
    );
}

// A body that is a single parameter reference is one Local node at the parameter's
// slot; the second parameter resolves to slot 1.
#[test]
fn a_parameter_reference_resolves_to_its_slot() {
    let src = "fn second(a: Word, b: Word) -> Word { b }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, root) = run_body(src);
    assert_eq!(root, 0);
    assert_eq!(
        nodes,
        vec![Node {
            kind: 2,
            arg: 1,
            lhs: 0,
            rhs: 0
        }]
    );
}

// The first parameter resolves to slot 0.
#[test]
fn the_first_parameter_resolves_to_slot_zero() {
    let src = "fn ident(x: Word) -> Word { x }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _) = run_body(src);
    assert_eq!(
        nodes,
        vec![Node {
            kind: 2,
            arg: 0,
            lhs: 0,
            rhs: 0
        }]
    );
}

// A zero literal round-trips (the record encoding does not lose a zero argument).
#[test]
fn a_zero_literal_round_trips() {
    let src = "fn zero() -> Word { 0 }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _) = run_body(src);
    assert_eq!(
        nodes,
        vec![Node {
            kind: 1,
            arg: 0,
            lhs: 0,
            rhs: 0
        }]
    );
}

// A single binary operator over two parameters is one BinOp node over two Locals.
#[test]
fn a_binary_operator_combines_two_operands() {
    let src = "fn sum(a: Word, b: Word) -> Word { a + b }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, root) = run_body(src);
    // Postorder: Local a (0), Local b (1), BinOp Add (2, lhs 0, rhs 1).
    assert_eq!(root, 2);
    assert_eq!(
        nodes[2],
        Node {
            kind: 3,
            arg: 1,
            lhs: 0,
            rhs: 1
        }
    );
}

// Multiplication binds tighter than addition: `a + b * c` parses as `a + (b * c)`.
#[test]
fn multiplication_binds_tighter_than_addition() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { a + b * c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, root) = run_body(src);
    // The root is Add; its right child is the Mul of b and c.
    let add = nodes[root as usize];
    assert_eq!(add.arg, 1); // Add
    let mul = nodes[add.rhs as usize];
    assert_eq!(mul.arg, 2); // Mul
}

// Subtraction is left-associative: `a - b - c` parses as `(a - b) - c`.
#[test]
fn subtraction_is_left_associative() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { a - b - c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, root) = run_body(src);
    // The root is a Sub whose left child is itself a Sub (left association).
    let outer = nodes[root as usize];
    assert_eq!(outer.arg, 3); // Sub
    let inner = nodes[outer.lhs as usize];
    assert_eq!(inner.arg, 3); // Sub
}

// Comparison binds loosest: `a + b == c` parses as `(a + b) == c`.
#[test]
fn comparison_binds_loosest() {
    let src = "fn f(a: Word, b: Word, c: Word) -> bool { a + b == c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, root) = run_body(src);
    let eq = nodes[root as usize];
    assert_eq!(eq.arg, 6); // Eq
    let add = nodes[eq.lhs as usize];
    assert_eq!(add.arg, 1); // Add
}

// A literal operand mixes with parameters across every arithmetic precedence level.
#[test]
fn a_mixed_precedence_expression_matches_the_reference() {
    let src = "fn f(a: Word, b: Word) -> Word { a * 2 + b / 3 - 1 }";
    assert_eq!(run_body(src), reference_body(src));
}

// Parentheses override precedence: `(a + b) * c` parses as `(a + b) * c`, so the root
// is the Mul and its left child is the Add.
#[test]
fn parentheses_override_precedence() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { (a + b) * c }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, root) = run_body(src);
    let mul = nodes[root as usize];
    assert_eq!(mul.arg, 2); // Mul
    let add = nodes[mul.lhs as usize];
    assert_eq!(add.arg, 1); // Add
}

// A parenthesised group on the right of a tighter operator: `a * (b + c)`.
#[test]
fn parentheses_on_the_right_group_the_addition() {
    let src = "fn f(a: Word, b: Word, c: Word) -> Word { a * (b + c) }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, root) = run_body(src);
    let mul = nodes[root as usize];
    assert_eq!(mul.arg, 2); // Mul
    let add = nodes[mul.rhs as usize];
    assert_eq!(add.arg, 1); // Add
}

// Nested and redundant parentheses collapse to the inner expression.
#[test]
fn nested_parentheses_collapse() {
    let src = "fn f(x: Word) -> Word { ((x)) }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, _) = run_body(src);
    assert_eq!(
        nodes,
        vec![Node {
            kind: 2,
            arg: 0,
            lhs: 0,
            rhs: 0
        }]
    );
}

// Two parenthesised groups combine under a single operator: `(a + b) * (c - d)`.
#[test]
fn two_parenthesised_groups_combine() {
    let src = "fn f(a: Word, b: Word, c: Word, d: Word) -> Word { (a + b) * (c - d) }";
    assert_eq!(run_body(src), reference_body(src));
    let (nodes, root) = run_body(src);
    let mul = nodes[root as usize];
    assert_eq!(mul.arg, 2); // Mul
    assert_eq!(nodes[mul.lhs as usize].arg, 1); // Add
    assert_eq!(nodes[mul.rhs as usize].arg, 3); // Sub
}
