//! Host language-trap policy is enforced before execution and across image changes.
#![cfg(feature = "compile")]
use keleusma::Arena;
use keleusma::bytecode::Module;
use keleusma::vm::{TrapPolicy, Vm, VmError};

fn compile(source: &str) -> Module {
    keleusma::compiler::compile(
        &keleusma::parser::parse(&keleusma::lexer::tokenize(source).unwrap()).unwrap(),
    )
    .unwrap()
}
fn policy_error<T>(result: Result<T, VmError>, detail: &str) {
    match result {
        Err(VmError::VerifyError(message)) => assert!(message.contains(detail), "{message}"),
        Err(other) => panic!("wrong rejection {other:?}"),
        Ok(_) => panic!("policy unexpectedly admitted the module"),
    }
}

#[cfg(feature = "verify")]
mod verified {
    use super::*;
    use keleusma::bytecode::{Op, Value};
    use keleusma::vm::{DEFAULT_ARENA_CAPACITY, OverflowPolicy, VmOptions, VmState};
    fn strict() -> VmOptions {
        VmOptions {
            trap_policy: TrapPolicy::RequireTrapFree,
            ..VmOptions::default()
        }
    }
    fn free() -> Module {
        compile("fn main() -> Word { 7 }")
    }
    fn trapping() -> Module {
        compile("fn main() -> Word { 7 / 0 }")
    }

    #[test]
    fn strict_construction_refuses_traps_and_unavailable_proof() {
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        policy_error(
            Vm::new_with_options(trapping(), &arena, strict()),
            "MayTrap",
        );
        let mut invalid = free();
        invalid.chunks[0].ops = vec![Op::PopN(1), Op::Return];
        policy_error(
            Vm::new_with_options(invalid, &arena, strict()),
            "InvalidModule",
        );
        let mut unknown = free();
        unknown.chunks[0].ops.push(Op::FixedDiv(0));
        policy_error(
            Vm::new_with_options(unknown, &arena, strict()),
            "Unsupported",
        );
        let mut large = free();
        large.chunks[0].ops = vec![Op::PushImmediate(4); keleusma::trap_analysis::TRAP_OPCODE_CAP];
        large.chunks[0].ops.push(Op::Return);
        policy_error(Vm::new_with_options(large, &arena, strict()), "Capacity");
        let (mut vm, warnings) = Vm::new_with_options(free(), &arena, strict()).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        assert!(matches!(
            vm.call(&[]).unwrap(),
            VmState::Finished(Value::Int(7))
        ));
    }

    #[test]
    fn default_policy_preserves_trap_permitting_execution() {
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(trapping(), &arena).unwrap();
        assert_eq!(vm.trap_policy(), TrapPolicy::Allow);
        assert!(matches!(vm.call(&[]), Err(VmError::DivisionByZero)));
    }

    #[test]
    fn serialized_and_view_loaders_apply_options() {
        for trapping_subject in [false, true] {
            let module = if trapping_subject { trapping() } else { free() };
            let mut bytes = rkyv::util::AlignedVec::<8>::new();
            bytes.extend_from_slice(&module.to_bytes().unwrap());
            for view in [false, true] {
                let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
                let result = if view {
                    Vm::view_bytes_with_options(&bytes, &arena, strict())
                } else {
                    Vm::load_bytes_with_options(&bytes, &arena, strict())
                };
                if trapping_subject {
                    policy_error(result, "MayTrap");
                } else {
                    let (mut vm, _) = result.unwrap();
                    assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
                    assert!(matches!(
                        vm.call(&[]).unwrap(),
                        VmState::Finished(Value::Int(7))
                    ));
                }
            }
        }
    }

    #[test]
    fn borrowed_and_trust_skip_routes_need_real_trap_proof() {
        for trapping_subject in [false, true] {
            let module = if trapping_subject { trapping() } else { free() };
            let mut bytes = rkyv::util::AlignedVec::<8>::new();
            bytes.extend_from_slice(&module.to_bytes().unwrap());
            for route in 0..4 {
                let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
                // These compiler-produced fixtures are structurally valid and fit the arena.
                let vm = unsafe {
                    match route {
                        0 => Vm::new_unchecked(module.clone(), &arena),
                        1 => Vm::load_bytes_unchecked(&bytes, &arena),
                        2 => Vm::view_bytes_unchecked(&bytes, &arena),
                        _ => Vm::view_bytes_zero_copy(&bytes, &arena),
                    }
                }
                .unwrap();
                let result = vm.with_trap_policy(TrapPolicy::RequireTrapFree);
                if trapping_subject {
                    policy_error(result, "MayTrap");
                } else {
                    let mut vm = result.unwrap();
                    assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
                    assert!(matches!(
                        vm.call(&[]).unwrap(),
                        VmState::Finished(Value::Int(7))
                    ));
                }
            }
        }
    }

    #[test]
    fn failed_policy_change_preserves_policy_and_suspended_state() {
        let module = compile(
            "fn unused() -> Word { 7 / 0 } yield main(r: Word) -> Word { let reply = yield r; reply }",
        );
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let mut vm = Vm::new(module, &arena).unwrap();
        assert!(matches!(
            vm.call(&[Value::Int(7)]).unwrap(),
            VmState::Yielded(Value::Int(7))
        ));
        policy_error(vm.set_trap_policy(TrapPolicy::RequireTrapFree), "MayTrap");
        assert_eq!(vm.trap_policy(), TrapPolicy::Allow);
        assert!(matches!(
            vm.resume(Value::Int(9)).unwrap(),
            VmState::Finished(Value::Int(9))
        ));
    }

    #[test]
    fn replacement_preserves_policy_and_rejects_before_state_changes() {
        for schema_bypass in [false, true] {
            let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
            let subject = compile("yield main(r: Word) -> Word { let reply = yield r; reply }");
            let (mut vm, _) = Vm::new_with_options(subject, &arena, strict()).unwrap();
            assert!(matches!(
                vm.call(&[Value::Int(7)]).unwrap(),
                VmState::Yielded(Value::Int(7))
            ));
            let rejected = if schema_bypass {
                vm.replace_module_unchecked(trapping(), vec![])
            } else {
                vm.replace_module(trapping(), vec![])
            };
            policy_error(rejected, "MayTrap");
            assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
            assert!(matches!(
                vm.resume(Value::Int(9)).unwrap(),
                VmState::Finished(Value::Int(9))
            ));
            vm.replace_module(free(), vec![]).unwrap();
            assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
            assert!(matches!(
                vm.call(&[]).unwrap(),
                VmState::Finished(Value::Int(7))
            ));
            let mut unknown = free();
            unknown.chunks[0].ops.push(Op::FixedDiv(0));
            policy_error(vm.replace_module_unchecked(unknown, vec![]), "Unsupported");
            assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        }
    }

    #[test]
    fn resets_breakpoints_errors_and_explicit_opt_out_preserve_the_selected_policy() {
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let module = compile("loop main(r: Word) -> Word { yield r }");
        let (mut vm, _) = Vm::new_with_options(module, &arena, strict()).unwrap();
        assert!(matches!(
            vm.call(&[Value::Int(7)]).unwrap(),
            VmState::Yielded(_)
        ));
        assert!(matches!(vm.resume(Value::Int(0)).unwrap(), VmState::Reset));
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        assert!(matches!(
            vm.resume(Value::Int(0)).unwrap(),
            VmState::Yielded(_)
        ));
        vm.reset_after_error();
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        vm.replace_module(free(), vec![]).unwrap();
        vm.set_breakpoint(0, 0);
        assert!(matches!(
            vm.call(&[]).unwrap(),
            VmState::BreakpointHit { .. }
        ));
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        assert!(matches!(
            vm.resume_from_breakpoint().unwrap(),
            VmState::Finished(Value::Int(7))
        ));
        vm.clear_breakpoints();
        vm.set_trap_policy(TrapPolicy::Allow).unwrap();
        vm.replace_module(trapping(), vec![]).unwrap();
        assert_eq!(vm.trap_policy(), TrapPolicy::Allow);
        assert!(matches!(vm.call(&[]), Err(VmError::DivisionByZero)));
    }

    #[test]
    fn strict_policy_retains_runtime_contract_guards_and_survives_error_reset() {
        let mut module = free();
        module.chunks[0].constants = vec![
            keleusma::bytecode::ConstValue::Bool(false),
            keleusma::bytecode::ConstValue::Int(1),
        ];
        module.chunks[0].ops = vec![
            Op::Const(0),
            Op::Const(1),
            Op::CheckedAdd,
            Op::PopN(2),
            Op::Return,
        ];
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let (mut vm, _) = Vm::new_with_options(module, &arena, strict()).unwrap();
        // Trap classification does not strengthen the typed verifier's numeric transfer.
        assert!(matches!(vm.call(&[]), Err(VmError::TypeError(_))));
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        vm.reset_after_error();
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        vm.replace_module(free(), vec![]).unwrap();
        assert!(matches!(
            vm.call(&[]).unwrap(),
            VmState::Finished(Value::Int(7))
        ));
    }

    #[test]
    fn overflow_policy_does_not_disable_trap_policy() {
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        let options = VmOptions {
            overflow_policy: OverflowPolicy::Warn,
            ..strict()
        };
        let mut module = free();
        module.wcet_cycles = u32::MAX;
        let (vm, warnings) = Vm::new_with_options(module, &arena, options).unwrap();
        assert_eq!(warnings.len(), 1);
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        let mut module = trapping();
        module.wcet_cycles = u32::MAX;
        policy_error(Vm::new_with_options(module, &arena, options), "MayTrap");
    }

    #[cfg(feature = "signatures")]
    #[test]
    fn signed_load_and_serialized_replacement_preserve_both_host_policies() {
        let signer = ed25519_dalek::SigningKey::from_bytes(&[91; 32]);
        let keys = [signer.verifying_key()];
        let good = keleusma::wire_format::module_to_signed_wire_bytes(&free(), &signer).unwrap();
        let bad = keleusma::wire_format::module_to_signed_wire_bytes(&trapping(), &signer).unwrap();
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        policy_error(
            Vm::load_signed_bytes_with_options(&bad, &arena, &keys, strict()),
            "MayTrap",
        );
        let (mut vm, _) =
            Vm::load_signed_bytes_with_options(&good, &arena, &keys, strict()).unwrap();
        assert_eq!(vm.verifying_keys_len(), 1);
        policy_error(vm.replace_module_from_bytes(&bad, vec![]), "MayTrap");
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        assert_eq!(vm.verifying_keys_len(), 1);
        assert!(matches!(
            vm.call(&[]).unwrap(),
            VmState::Finished(Value::Int(7))
        ));
        vm.replace_module_from_bytes(&good, vec![]).unwrap();
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        assert!(
            vm.replace_module_from_bytes(&free().to_bytes().unwrap(), vec![])
                .is_err()
        );
        assert_eq!(vm.verifying_keys_len(), 1);
        let other_signer = ed25519_dalek::SigningKey::from_bytes(&[92; 32]);
        let unauthenticated =
            keleusma::wire_format::module_to_signed_wire_bytes(&free(), &other_signer).unwrap();
        assert!(matches!(
            vm.replace_module_from_bytes(&unauthenticated, vec![]),
            Err(VmError::LoadError(_))
        ));
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        vm.clear_verifying_keys();
        policy_error(
            vm.replace_module_from_bytes(&trapping().to_bytes().unwrap(), vec![]),
            "MayTrap",
        );
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        let vm = unsafe { Vm::load_signed_bytes_unchecked(&bad, &arena, &keys) }.unwrap();
        policy_error(vm.with_trap_policy(TrapPolicy::RequireTrapFree), "MayTrap");
    }
    #[cfg(all(feature = "signatures", feature = "encryption"))]
    #[test]
    fn encrypted_load_enforces_trap_policy_after_authentication() {
        let signer = ed25519_dalek::SigningKey::from_bytes(&[93; 32]);
        let keys = [signer.verifying_key()];
        let recipient = [94; 32];
        let public = keleusma::encryption::public_key_from_private(&recipient);
        let good = keleusma::wire_format::module_to_encrypted_signed_wire_bytes(
            &free(),
            &signer,
            &public,
            &[95; 32],
        )
        .unwrap();
        let bad = keleusma::wire_format::module_to_encrypted_signed_wire_bytes(
            &trapping(),
            &signer,
            &public,
            &[96; 32],
        )
        .unwrap();
        let arena = Arena::with_capacity(DEFAULT_ARENA_CAPACITY);
        policy_error(
            Vm::load_encrypted_signed_bytes_with_options(&bad, &arena, &keys, &recipient, strict()),
            "MayTrap",
        );
        let (mut vm, _) = Vm::load_encrypted_signed_bytes_with_options(
            &good,
            &arena,
            &keys,
            &recipient,
            strict(),
        )
        .unwrap();
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        assert_eq!(vm.verifying_keys_len(), 1);
        assert!(matches!(
            vm.call(&[]).unwrap(),
            VmState::Finished(Value::Int(7))
        ));
        let signed_bad =
            keleusma::wire_format::module_to_signed_wire_bytes(&trapping(), &signer).unwrap();
        policy_error(vm.replace_module_from_bytes(&signed_bad, vec![]), "MayTrap");
        assert_eq!(vm.trap_policy(), TrapPolicy::RequireTrapFree);
        assert!(
            Vm::load_encrypted_signed_bytes_with_options(&good, &arena, &keys, &[97; 32], strict())
                .is_err()
        );
        let vm =
            unsafe { Vm::load_encrypted_signed_bytes_unchecked(&bad, &arena, &keys, &recipient) }
                .unwrap();
        policy_error(vm.with_trap_policy(TrapPolicy::RequireTrapFree), "MayTrap");
    }
}

#[cfg(not(feature = "verify"))]
#[test]
fn unavailable_verification_cannot_be_replaced_by_a_host_attestation() {
    let arena = Arena::with_capacity(keleusma::vm::DEFAULT_ARENA_CAPACITY);
    // This fixture is known valid and bounded, permitting the existing unsafe constructor.
    let mut vm = unsafe { Vm::new_unchecked(compile("fn main() -> Word { 7 }"), &arena) }.unwrap();
    policy_error(
        vm.set_trap_policy(TrapPolicy::RequireTrapFree),
        "verification support",
    );
    assert_eq!(vm.trap_policy(), TrapPolicy::Allow);
    vm.set_trap_policy(TrapPolicy::Allow).unwrap();
    #[cfg(feature = "signatures")]
    {
        let signer = ed25519_dalek::SigningKey::from_bytes(&[81; 32]);
        let keys = [signer.verifying_key()];
        let module = compile("fn main() -> Word { 7 }");
        let bytes = keleusma::wire_format::module_to_signed_wire_bytes(&module, &signer).unwrap();
        let vm = unsafe { Vm::load_signed_bytes_unchecked(&bytes, &arena, &keys) }.unwrap();
        policy_error(
            vm.with_trap_policy(TrapPolicy::RequireTrapFree),
            "verification support",
        );
        #[cfg(feature = "encryption")]
        {
            let private = [82; 32];
            let public = keleusma::encryption::public_key_from_private(&private);
            let bytes = keleusma::wire_format::module_to_encrypted_signed_wire_bytes(
                &module, &signer, &public, &[83; 32],
            )
            .unwrap();
            let vm = unsafe {
                Vm::load_encrypted_signed_bytes_unchecked(&bytes, &arena, &keys, &private)
            }
            .unwrap();
            policy_error(
                vm.with_trap_policy(TrapPolicy::RequireTrapFree),
                "verification support",
            );
        }
    }
}
