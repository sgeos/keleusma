extern crate alloc;
use alloc::format;

use crate::address::Address;
use crate::bytecode::GenericValue;
use crate::float::Float;
use crate::vm::{GenericVm, VmError};
use crate::word::Word;

/// Helper: validate that the argument count matches `expected` and
/// produce a uniform error message otherwise.
fn check_arity<W: Word, F: Float>(
    name: &str,
    expected: usize,
    args: &[GenericValue<W, F>],
) -> Result<(), VmError> {
    if args.len() != expected {
        return Err(VmError::NativeError(format!(
            "{}: expected {} argument{}, got {}",
            name,
            expected,
            if expected == 1 { "" } else { "s" },
            args.len()
        )));
    }
    Ok(())
}

/// Debug print a value. Returns Unit. In no_std this is a no-op; the
/// host can override with a closure using `register_native_closure` if
/// output is desired.
fn native_println<W: Word, F: Float>(
    args: &[GenericValue<W, F>],
) -> Result<GenericValue<W, F>, VmError> {
    check_arity("println", 1, args)?;
    // No-op in no_std. The value is consumed but not printed.
    Ok(GenericValue::Unit)
}

/// Register the bundled utility native functions on the VM.
///
/// Registers: `println`.
///
/// V0.2.0 removed the script-side text-composition machinery
/// (`to_string`, `concat`, `slice`, `length`). Hosts that want
/// formatting register their own native functions per format shape;
/// the language no longer ships a bundled text-utility library.
///
/// `println` is retained as a debug-print primitive that operates on
/// any value type. The bundled implementation is a no-op suitable for
/// `no_std` hosts; hosts that want output register a closure that
/// writes to their own sink.
pub fn register_utility_natives<'a, 'arena, W: Word, A: Address, F: Float>(
    vm: &mut GenericVm<'a, 'arena, W, A, F>,
) {
    vm.register_native("println", native_println::<W, F>);
}

#[cfg(all(test, feature = "compile", feature = "verify", feature = "floats"))]
mod tests {
    use super::*;
    use crate::bytecode::Value;
    use crate::compiler::compile;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use crate::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmState};

    fn run_with_utilities(src: &str, arena: &keleusma_arena::Arena) -> Value {
        let tokens = tokenize(src).expect("lex error");
        let program = parse(&tokens).expect("parse error");
        let module = compile(&program).expect("compile error");
        let mut vm = Vm::new(module, arena).unwrap();
        register_utility_natives(&mut vm);
        vm.register_library(crate::stddsl::Math);
        match vm.call(&[]).unwrap() {
            VmState::Finished(v) => v,
            VmState::Yielded(v) => panic!("unexpected yield: {:?}", v),
            VmState::Reset => panic!("unexpected reset"),
        }
    }

    #[test]
    fn println_returns_unit() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let val = run_with_utilities("use println\nfn main() -> () { println(42) }", &arena);
        assert_eq!(val, Value::Unit);
    }

    #[test]
    fn sqrt_value() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let val = run_with_utilities(
            "use math::sqrt\nfn main() -> Float { math::sqrt(9.0) }",
            &arena,
        );
        assert_eq!(val, Value::Float(3.0));
    }

    #[test]
    fn floor_value() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let val = run_with_utilities(
            "use math::floor\nfn main() -> Float { math::floor(3.7) }",
            &arena,
        );
        assert_eq!(val, Value::Float(3.0));
    }

    #[test]
    fn log2_value() {
        let arena = keleusma_arena::Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let val = run_with_utilities(
            "use math::log2\nfn main() -> Float { math::log2(8.0) }",
            &arena,
        );
        assert_eq!(val, Value::Float(3.0));
    }
}
