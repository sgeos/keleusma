//! Generated core modules, their lowering to increment A's core instructions, their
//! translation to Keleusma source, and the structural predicates that classify a
//! disagreement.
//!
//! # The generated fragment
//!
//! A module is generated as structured statements, not as raw core instructions. Each
//! statement has one lowering to the core of `docs/proofs/rocq/core_bounds/Core.v` and one
//! translation to Keleusma source, chosen so that the two programs take the same branches
//! and yield the same outputs on the same replies. The table gives both. In it `v` is the
//! main parameter `x`, which is core local 0, inside the main block, and the function
//! parameter `a`, which is core local 1, inside a function.
//!
//! | statement | core | Keleusma |
//! |---|---|---|
//! | `Work(op, k)` | `Load v; Const k; op; Store 2` | `let w = v op k;` |
//! | `Emit(c)` | `Const c; Yield; Store 0` | `yield c;` |
//! | `YIf(c, t, e)` | `Const c; Yield; Store 0; Load 0; If t e` | `if (yield c) != 0 { t } else { e };` |
//! | `LIf(t, e)` | `Load v; If t e` | `if v != 0 { t } else { e };` |
//! | `For(n, b)` | `LoopN n b` | `for iD in 0..n { b };` |
//! | `Call(j)` in main | `Load 0; Store 1; Call j` | `fj(x, x);` |
//! | `Call(j)` in a function | `Call j` | `fj(a, a);` |
//!
//! Every function takes a second parameter `b` that it never reads. It exists so that a
//! call dispatches at least as many operations before entering its callee as the core
//! spends passing the argument, which the domination check of the lockstep relation needs.
//! With one argument a call from main dispatched two operations against the core's three,
//! which a first run on 2026-11-08 reported as a lockstep disagreement.
//!
//! The alignment rests on three facts. The Keleusma runtime writes every resume value to
//! the parameter of `loop main`, and the core stores every reply to local 0, so `x` and
//! local 0 agree. Main passes `x` to a call and stores it to local 1 first, and a function
//! passes its own `a` on, so inside one call from main `a` and local 1 agree. Each yield
//! sends a constant unique to its site, so a divergence of branches shows as a divergence
//! of outputs. The lockstep relation of `main.rs` tests all three on every run.
//!
//! The generator never produces `Alloc`, so memory figures are not compared, and it never
//! produces an unbalanced stack. Those limits are stated in the README.
//!
//! # Recursion in this file
//!
//! The walkers below recurse over statement trees whose nesting the generator bounds by
//! [`MAX_DEPTH`], so their depth is bounded by construction.

use crate::rng::Rng;
use std::fmt::Write as _;

/// The deepest statement nesting the generator produces, counted from the body at zero.
pub const MAX_DEPTH: u32 = 3;

/// The two function categories of the core, `fn` and `yield`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// An atomic total `fn`.
    Fn,
    /// A non-atomic total `yield` function.
    Yield,
}

/// The arithmetic of a `Work` statement.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArOp {
    Add,
    Sub,
    Lt,
}

/// A structured statement. See the module documentation for its two meanings.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Stmt {
    Work(ArOp, u32),
    Emit(u32),
    YIf(u32, Vec<Stmt>, Vec<Stmt>),
    LIf(Vec<Stmt>, Vec<Stmt>),
    For(u32, Vec<Stmt>),
    Call(usize),
}

/// A generated function.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Func {
    pub kind: Kind,
    pub body: Vec<Stmt>,
}

/// A generated module, functions in index order and the `loop main` body.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CoreModule {
    pub funcs: Vec<Func>,
    pub main: Vec<Stmt>,
}

/// Where a statement list sits, which fixes the meaning of `v`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ctx {
    Main,
    Func,
}

// ---------------------------------------------------------------------------------------
// Generation.
// ---------------------------------------------------------------------------------------

/// Per-module switches, drawn once per module so that each structural rule is exercised
/// often enough to be measured.
struct Knobs {
    /// Calls in function `i` may target any function, so recursion and forward calls occur.
    any_call: bool,
    /// An `fn` may contain a yield.
    fn_yield: bool,
    /// The percentage of loops given zero iterations.
    zero_loops: u64,
}

struct Gen<'a> {
    rng: &'a mut Rng,
    knobs: Knobs,
    nfuncs: usize,
    next_site: u32,
}

impl Gen<'_> {
    fn site(&mut self) -> u32 {
        self.next_site += 1;
        self.next_site
    }

    /// A statement list for the function `owner`, or for main when `owner` is `None`.
    fn stmts(&mut self, owner: Option<(usize, Kind)>, depth: u32, max_len: u64) -> Vec<Stmt> {
        let len = self.rng.below(max_len + 1);
        (0..len).map(|_| self.stmt(owner, depth)).collect()
    }

    fn stmt(&mut self, owner: Option<(usize, Kind)>, depth: u32) -> Stmt {
        let can_call = match owner {
            None => self.nfuncs > 0,
            Some((i, _)) => i > 0 || (self.knobs.any_call && self.nfuncs > 0),
        };
        let may_yield = match owner {
            None | Some((_, Kind::Yield)) => true,
            Some((_, Kind::Fn)) => self.knobs.fn_yield,
        };
        let nested = depth < MAX_DEPTH;
        // Weights of Work, Emit, YIf, LIf, For, Call.
        let w = match owner {
            None => [10, 25, 15, 20, 15, 15],
            Some((_, Kind::Yield)) => [25, 20, 10, 20, 15, 10],
            Some((_, Kind::Fn)) => [40, 8, 4, 20, 20, 10],
        };
        let weights = [
            w[0],
            if may_yield { w[1] } else { 0 },
            if may_yield && nested { w[2] } else { 0 },
            if nested { w[3] } else { 0 },
            if nested { w[4] } else { 0 },
            if can_call { w[5] } else { 0 },
        ];
        match self.rng.weighted(&weights) {
            0 => {
                let op = [ArOp::Add, ArOp::Sub, ArOp::Lt][self.rng.below(3) as usize];
                Stmt::Work(op, self.rng.below(9) as u32 + 1)
            }
            1 => Stmt::Emit(self.site()),
            2 => {
                let c = self.site();
                Stmt::YIf(
                    c,
                    self.stmts(owner, depth + 1, 2),
                    self.stmts(owner, depth + 1, 2),
                )
            }
            3 => Stmt::LIf(
                self.stmts(owner, depth + 1, 2),
                self.stmts(owner, depth + 1, 2),
            ),
            4 => {
                let n = if self.rng.chance(self.knobs.zero_loops) {
                    0
                } else {
                    self.rng.below(3) as u32 + 1
                };
                Stmt::For(n, self.stmts(owner, depth + 1, 2))
            }
            _ => {
                let bound = match owner {
                    Some((i, _)) if !self.knobs.any_call => i,
                    _ => self.nfuncs,
                };
                Stmt::Call(self.rng.below(bound as u64) as usize)
            }
        }
    }
}

/// One generated module, deterministic in the state of `rng`.
pub fn generate(rng: &mut Rng) -> CoreModule {
    let knobs = Knobs {
        any_call: rng.chance(20),
        fn_yield: rng.chance(15),
        zero_loops: if rng.chance(30) { 40 } else { 5 },
    };
    let nfuncs = rng.below(4) as usize;
    let mut g = Gen {
        rng,
        knobs,
        nfuncs,
        next_site: 0,
    };
    let mut funcs = Vec::with_capacity(nfuncs);
    for i in 0..nfuncs {
        let kind = if g.rng.chance(50) {
            Kind::Fn
        } else {
            Kind::Yield
        };
        let body = g.stmts(Some((i, kind)), 0, 3);
        funcs.push(Func { kind, body });
    }
    let mut main = g.stmts(None, 0, 4);
    if main.is_empty() {
        main.push(g.stmt(None, 0));
    }
    CoreModule { funcs, main }
}

// ---------------------------------------------------------------------------------------
// Size, to keep the unary model fast and the reply strings long enough.
// ---------------------------------------------------------------------------------------

/// An over-approximation of the instructions and yields one cycle may execute, with calls
/// expanded and each call cycle cut after `fuel` levels. Saturating.
pub fn cycle_size(m: &CoreModule) -> (u64, u64) {
    fn go(m: &CoreModule, s: &[Stmt], fuel: u32) -> (u64, u64) {
        let mut ops = 0u64;
        let mut ys = 0u64;
        for st in s {
            let (o, y) = match st {
                Stmt::Work(..) => (4, 0),
                Stmt::Emit(_) => (3, 1),
                Stmt::YIf(_, t, e) | Stmt::LIf(t, e) => {
                    let (a, b) = (go(m, t, fuel), go(m, e, fuel));
                    let y0 = u64::from(matches!(st, Stmt::YIf(..)));
                    (5 + a.0.max(b.0), y0 + a.1.max(b.1))
                }
                Stmt::For(n, b) => {
                    let (o, y) = go(m, b, fuel);
                    (1 + u64::from(*n) * (o + 1), u64::from(*n) * y)
                }
                Stmt::Call(j) => {
                    if fuel == 0 {
                        (3, 0)
                    } else {
                        let (o, y) = go(m, &m.funcs[*j].body, fuel - 1);
                        (3 + o, y)
                    }
                }
            };
            ops = ops.saturating_add(o);
            ys = ys.saturating_add(y);
        }
        (ops, ys)
    }
    go(m, &m.main, 4)
}

// ---------------------------------------------------------------------------------------
// Lowering to the core, in the text format of model/model_main.ml.
// ---------------------------------------------------------------------------------------

fn lower_stmts(s: &[Stmt], ctx: Ctx, out: &mut String) {
    let v = if ctx == Ctx::Main { 0 } else { 1 };
    out.push_str("[ ");
    for st in s {
        match st {
            Stmt::Work(op, k) => {
                let o = match op {
                    ArOp::Add => "ADD",
                    ArOp::Sub => "SUB",
                    ArOp::Lt => "LT",
                };
                let _ = write!(out, "LD {v} C {k} {o} ST 2 ");
            }
            Stmt::Emit(c) => {
                let _ = write!(out, "C {c} Y ST 0 ");
            }
            Stmt::YIf(c, t, e) => {
                let _ = write!(out, "C {c} Y ST 0 LD 0 IF ");
                lower_stmts(t, ctx, out);
                lower_stmts(e, ctx, out);
            }
            Stmt::LIf(t, e) => {
                let _ = write!(out, "LD {v} IF ");
                lower_stmts(t, ctx, out);
                lower_stmts(e, ctx, out);
            }
            Stmt::For(n, b) => {
                let _ = write!(out, "LOOP {n} ");
                lower_stmts(b, ctx, out);
            }
            Stmt::Call(j) => {
                if ctx == Ctx::Main {
                    let _ = write!(out, "LD 0 ST 1 CALL {j} ");
                } else {
                    let _ = write!(out, "CALL {j} ");
                }
            }
        }
    }
    out.push_str("] ");
}

/// The case text the model reads, without its `case`, `cycles` and `host` lines.
pub fn lower(m: &CoreModule) -> String {
    let mut out = String::new();
    for f in &m.funcs {
        out.push_str(if f.kind == Kind::Fn {
            "func F "
        } else {
            "func Y "
        });
        lower_stmts(&f.body, Ctx::Func, &mut out);
        out.push('\n');
    }
    out.push_str("main ");
    lower_stmts(&m.main, Ctx::Main, &mut out);
    out.push('\n');
    out
}

// ---------------------------------------------------------------------------------------
// Translation to Keleusma source.
// ---------------------------------------------------------------------------------------

/// The deliberate translation defect of the negative control is `swap`, which exchanges
/// the arms of every conditional in the Keleusma program only.
fn translate_stmts(s: &[Stmt], ctx: Ctx, depth: u32, swap: bool, out: &mut String) {
    let v = if ctx == Ctx::Main { "x" } else { "a" };
    for st in s {
        match st {
            Stmt::Work(op, k) => {
                let o = match op {
                    ArOp::Add => "+",
                    ArOp::Sub => "-",
                    ArOp::Lt => "<",
                };
                let _ = write!(out, "let w = {v} {o} {k}; ");
            }
            Stmt::Emit(c) => {
                let _ = write!(out, "yield {c}; ");
            }
            Stmt::YIf(_, t, e) | Stmt::LIf(t, e) => {
                match st {
                    Stmt::YIf(c, ..) => {
                        let _ = write!(out, "if (yield {c}) != 0 {{ ");
                    }
                    _ => {
                        let _ = write!(out, "if {v} != 0 {{ ");
                    }
                }
                let (first, second) = if swap { (e, t) } else { (t, e) };
                translate_stmts(first, ctx, depth + 1, swap, out);
                out.push_str("} else { ");
                translate_stmts(second, ctx, depth + 1, swap, out);
                out.push_str("}; ");
            }
            Stmt::For(n, b) => {
                let _ = write!(out, "for i{depth} in 0..{n} {{ ");
                translate_stmts(b, ctx, depth + 1, swap, out);
                out.push_str("}; ");
            }
            Stmt::Call(j) => {
                let _ = write!(out, "f{j}({v}, {v}); ");
            }
        }
    }
}

/// The Keleusma source of `m`.
pub fn translate(m: &CoreModule, swap: bool) -> String {
    let mut out = String::new();
    for (i, f) in m.funcs.iter().enumerate() {
        let kw = if f.kind == Kind::Fn { "fn" } else { "yield" };
        let _ = write!(out, "{kw} f{i}(a: Word, b: Word) -> Word {{ ");
        translate_stmts(&f.body, Ctx::Func, 0, swap, &mut out);
        out.push_str("a }\n");
    }
    out.push_str("loop main(x: Word) -> Word { ");
    translate_stmts(&m.main, Ctx::Main, 0, swap, &mut out);
    out.push_str("x }\n");
    out
}

// ---------------------------------------------------------------------------------------
// Structural predicates. These restate the rules each side documents, and they are
// computed from the generated module alone, never from either side's verdict.
// ---------------------------------------------------------------------------------------

fn visit<'a>(s: &'a [Stmt], f: &mut impl FnMut(&'a Stmt)) {
    for st in s {
        f(st);
        match st {
            Stmt::YIf(_, t, e) | Stmt::LIf(t, e) => {
                visit(t, f);
                visit(e, f);
            }
            Stmt::For(_, b) => visit(b, f),
            _ => {}
        }
    }
}

/// The functions each function calls, syntactically, under any loop count.
pub fn callees(s: &[Stmt]) -> Vec<usize> {
    let mut out = Vec::new();
    visit(s, &mut |st| {
        if let Stmt::Call(j) = st {
            out.push(*j);
        }
    });
    out
}

/// Whether `s` contains a yield anywhere, syntactically.
pub fn has_syntactic_yield(s: &[Stmt]) -> bool {
    let mut found = false;
    visit(s, &mut |st| {
        found |= matches!(st, Stmt::Emit(_) | Stmt::YIf(..))
    });
    found
}

/// Whether the call graph of the functions has a cycle, by Kahn's algorithm.
pub fn call_graph_cyclic(m: &CoreModule) -> bool {
    topo_order(m).is_none()
}

/// An order of the functions in which every callee precedes its callers, when the call
/// graph is acyclic.
fn topo_order(m: &CoreModule) -> Option<Vec<usize>> {
    let n = m.funcs.len();
    let edges: Vec<Vec<usize>> = m.funcs.iter().map(|f| callees(&f.body)).collect();
    // Remaining callee count per function, with duplicate edges counted once.
    let mut pending: Vec<usize> = edges
        .iter()
        .map(|e| {
            let mut d = e.clone();
            d.sort_unstable();
            d.dedup();
            d.len()
        })
        .collect();
    let mut done = vec![false; n];
    let mut order = Vec::with_capacity(n);
    // At most n rounds, each placing at least one function or stopping.
    for _ in 0..n {
        let Some(next) = (0..n).find(|&i| !done[i] && pending[i] == 0) else {
            break;
        };
        done[next] = true;
        order.push(next);
        for (i, e) in edges.iter().enumerate() {
            if !done[i] && e.contains(&next) {
                pending[i] -= 1;
            }
        }
    }
    (order.len() == n).then_some(order)
}

/// Whether some function calls a function of equal or greater index, which the model's
/// call rule rejects even when the graph is acyclic.
pub fn has_forward_call(m: &CoreModule) -> bool {
    m.funcs
        .iter()
        .enumerate()
        .any(|(i, f)| callees(&f.body).iter().any(|&j| j >= i))
}

fn remap(s: &[Stmt], new_index: &[usize]) -> Vec<Stmt> {
    s.iter()
        .map(|st| match st {
            Stmt::Call(j) => Stmt::Call(new_index[*j]),
            Stmt::YIf(c, t, e) => Stmt::YIf(*c, remap(t, new_index), remap(e, new_index)),
            Stmt::LIf(t, e) => Stmt::LIf(remap(t, new_index), remap(e, new_index)),
            Stmt::For(n, b) => Stmt::For(*n, remap(b, new_index)),
            other => other.clone(),
        })
        .collect()
}

/// The same module with its functions reordered so that every call is backward, when the
/// call graph is acyclic. The model judges it by the same rules except the order rule.
pub fn topo_reordered(m: &CoreModule) -> Option<CoreModule> {
    let order = topo_order(m)?;
    let mut new_index = vec![0; m.funcs.len()];
    for (new, &old) in order.iter().enumerate() {
        new_index[old] = new;
    }
    let funcs = order
        .iter()
        .map(|&old| Func {
            kind: m.funcs[old].kind,
            body: remap(&m.funcs[old].body, &new_index),
        })
        .collect();
    Some(CoreModule {
        funcs,
        main: remap(&m.main, &new_index),
    })
}

/// The conservative productivity rule the shipping verifier documents. A path yields when
/// it passes a yield, or a call of an always-yielding function, outside every loop. A loop
/// body may be skipped, because the bounded loop tests its exit before its body.
fn yields_on_every_path(s: &[Stmt], always: &[bool]) -> bool {
    let mut yielded = false;
    for st in s {
        yielded |= match st {
            Stmt::Emit(_) | Stmt::YIf(..) => true,
            Stmt::LIf(t, e) => yields_on_every_path(t, always) && yields_on_every_path(e, always),
            Stmt::Call(j) => always[*j],
            Stmt::Work(..) | Stmt::For(..) => false,
        };
    }
    yielded
}

/// The always-yielding functions under the conservative rule, by a monotone fixpoint.
pub fn always_yielding(m: &CoreModule) -> Vec<bool> {
    let mut always = vec![false; m.funcs.len()];
    // Each round adds at least one function or stops, so n + 1 rounds suffice.
    for _ in 0..=m.funcs.len() {
        let mut changed = false;
        for (i, f) in m.funcs.iter().enumerate() {
            if !always[i] && f.kind == Kind::Yield && yields_on_every_path(&f.body, &always) {
                always[i] = true;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    always
}

/// The rules on which the shipping pipeline is documented to be stricter than the model,
/// each with the text its rejection is documented to carry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strict {
    /// An `fn` calls a `yield` function, refused by the category-call discipline.
    FnCallsYield,
    /// An `fn` contains a yield that the model shows can never run, refused syntactically.
    FnDeadYield,
    /// A `yield` function contains no yield and calls no always-yielding function.
    YieldFnWithoutYield,
    /// The main block yields on every path only through a loop body, or through a function
    /// that yields on every path only through one.
    LoopProductivity,
}

impl Strict {
    /// Substrings of the shipping diagnostics for this rule, one of which a rejection by it
    /// carries.
    pub fn diagnostics(self) -> &'static [&'static str] {
        match self {
            Strict::FnCallsYield => &["may not call a non-atomic total"],
            Strict::FnDeadYield => &["Func block contains"],
            Strict::YieldFnWithoutYield => &["Reentrant block must contain at least one Yield"],
            Strict::LoopProductivity => &[
                "main: productivity violation",
                "main: Stream block must contain at least one Yield",
            ],
        }
    }

    /// A short name for reports.
    pub fn name(self) -> &'static str {
        match self {
            Strict::FnCallsYield => "fn-calls-yield-function",
            Strict::FnDeadYield => "fn-with-unreachable-yield",
            Strict::YieldFnWithoutYield => "yield-function-without-yield",
            Strict::LoopProductivity => "productivity-through-loop",
        }
    }
}

/// The stricter rules that fire on `m`, under the shipping pipeline's documented rules.
pub fn strict_rules(m: &CoreModule) -> Vec<Strict> {
    let mut out = Vec::new();
    let always = always_yielding(m);
    if m.funcs.iter().any(|f| {
        f.kind == Kind::Fn
            && callees(&f.body)
                .iter()
                .any(|&j| m.funcs[j].kind == Kind::Yield)
    }) {
        out.push(Strict::FnCallsYield);
    }
    if m.funcs
        .iter()
        .any(|f| f.kind == Kind::Fn && has_syntactic_yield(&f.body))
    {
        out.push(Strict::FnDeadYield);
    }
    if m.funcs.iter().any(|f| {
        f.kind == Kind::Yield
            && !has_syntactic_yield(&f.body)
            && !callees(&f.body).iter().any(|&j| always[j])
    }) {
        out.push(Strict::YieldFnWithoutYield);
    }
    if !yields_on_every_path(&m.main, &always) {
        out.push(Strict::LoopProductivity);
    }
    out
}

/// Whether the main block contains a call, at any depth.
pub fn main_calls(m: &CoreModule) -> bool {
    !callees(&m.main).is_empty()
}

/// The features of `m` that the coverage counts of the report name.
pub fn features(m: &CoreModule) -> Vec<&'static str> {
    fn depth(s: &[Stmt]) -> u32 {
        s.iter()
            .map(|st| match st {
                Stmt::YIf(_, t, e) | Stmt::LIf(t, e) => 1 + depth(t).max(depth(e)),
                Stmt::For(_, b) => 1 + depth(b),
                _ => 0,
            })
            .max()
            .unwrap_or(0)
    }
    fn yield_in_loop(s: &[Stmt], inside: bool) -> bool {
        s.iter().any(|st| match st {
            Stmt::Emit(_) => inside,
            Stmt::YIf(_, t, e) => inside || yield_in_loop(t, inside) || yield_in_loop(e, inside),
            Stmt::LIf(t, e) => yield_in_loop(t, inside) || yield_in_loop(e, inside),
            Stmt::For(n, b) => yield_in_loop(b, inside || *n > 0),
            _ => false,
        })
    }
    let mut out = Vec::new();
    let all: Vec<&[Stmt]> = m
        .funcs
        .iter()
        .map(|f| f.body.as_slice())
        .chain(std::iter::once(m.main.as_slice()))
        .collect();
    if callees(&m.main)
        .iter()
        .any(|&j| m.funcs[j].kind == Kind::Yield)
    {
        out.push("main calls a yield function");
    }
    if !has_syntactic_yield(&m.main) {
        out.push("main yields only through delegation");
    }
    let mut zero = false;
    for s in &all {
        visit(s, &mut |st| zero |= matches!(st, Stmt::For(0, _)));
    }
    if zero {
        out.push("a loop of zero iterations");
    }
    if all.iter().any(|s| yield_in_loop(s, false)) {
        out.push("a yield inside a loop");
    }
    if all.iter().any(|s| depth(s) >= MAX_DEPTH) {
        out.push("nesting depth three");
    }
    if m.funcs
        .iter()
        .filter(|f| f.kind == Kind::Yield)
        .any(|f| !callees(&f.body).is_empty())
    {
        out.push("a yield function that calls");
    }
    if has_forward_call(m) {
        out.push("a forward call");
    }
    out
}
