# Workstream I, language constraints native lowering surfaced

> **Navigation** [Decisions](./README.md) | [Order 6 plan](./ORDER_6_PLAN.md) | [V0.3.X roadmap](../roadmap/V0_3_X_ROADMAP.md)

The roadmap's Workstream I is language feature additions and revisions, in the
same co-evolution posture as V0.2.x, as native lowering surfaces constraints.
The language, the compiler and the verifier belong to the V0.2.X line, so this
line's part is to find each constraint, measure it, and send it to that line.
This record lists every constraint native lowering has surfaced through
Order 6, with the evidence, who owns the decision, and what native code does
meanwhile.

| Constraint | Evidence | Owner | Native behaviour meanwhile |
|---|---|---|---|
| A local holds `Unit` after `Reset` on one path and a value on another | Since 2026-10-29, 2 of 30 corpus coroutines need a run-time kind. `14_frame_log.kel` needs a kind guard, which an empty start for private slots removes, and `interp.kel` reads an operand that may hold more than one kind. The ten `piano_roll` coroutines counted before were this line's census counting stack entries no instruction reads. See `ORDER_6_RULINGS_PLAN.md` | V0.2.X for both. The guard waits on question 20. In `interp.kel` the reference compiler emits an unreachable wrapping default after a fully covered checked arithmetic match, requested in sequence 136 | The coroutine keeps a run-time kind there, and `LowerOptions::static_kinds` refuses it |
| A composite built inside a loop and yielded from it | `13_telemetry_stream.kel` is refused by the coroutine route, since reusing the construction's fixed offset would hand the host the next iteration's bytes | V0.2.X, a language rule or a copy at the yield | Refused, not miscompiled |
| The verifier's text pass refuses a comparison of two `Word` call results | The lexer's `intern_id`, chunk 5 instruction 83. The lexer exports no composed cost claim. See `ORDER_4_COMPLETION.md` | V0.2.X, the verifier | The claim is reported as not established |
| A `Text` crossing the host boundary has no native representation | A `Text` from a call, a parameter, a native or a field is refused in a comparison, and a `Text` field is refused in a composite. See `TEXT_FIELD_VERDICT.md` and `text_comparison.rs` | The operator, a host string ABI | Literals compare by content, everything else refuses |
| Opaque host handles have no flat layout | Refused on every native route | The operator, a host reference ABI | Refused |
| Native WCET, hard or best effort | Roadmap open decision 1 | The operator | Best effort, in bytecode cost units |
| The bytecode half of the flat-machine ISA | Roadmap open decision 3 | The operator | The flat machine is native only |
| `RUNTIME_FAULTS.md` native default values | Superseded by the 2026-10-20 ruling that only `Trap` traps | V0.2.X, the specification | Native code follows the bytecode `Trap` |
| `schema_hash` covers slot names and visibility, not types | `native_hot_swap.rs` applies the same rule as `replace_module` | V0.2.X, `compute_schema_hash` | The same rule in both shapes |

Each row was also an entry in this line's open question list,
[`v0.3.0-QUESTIONS.md`](../process/handoffs/v0.3.0-QUESTIONS.md), and every
one has since been answered. The table above records the constraints as Order
6 found them, and the status below supersedes its last two columns.

## Status on 2026-10-30

| Constraint | Status |
|---|---|
| `Unit` after `Reset` on one path | The operator answered question 20 with option a, a narrowed definite-initialization rule. V0.2.X's checked default change, absorbed in absorption 116, made `interp.kel` static. Resolved. V0.2.X's widening, absorbed in absorption 124, gives every private slot its type's zero, and no corpus instruction needs a run-time kind |
| A composite built in a loop and yielded | Resolved on the coroutine route on 2026-10-31. The operator confirmed first hand that such a composite may be yielded, question 24, and the entry stream now copies each yielded body into a slot it keeps until the `Reset`. `13_telemetry_stream.kel` lowers there and its bodies agree with the virtual machine while held. A composite built in a callee's loop and yielded through delegation is still refused, and the module route, which does not copy, still refuses the shape |
| Two `Word` call results compared | Resolved. V0.2.X's callee-aware bound arrived in absorption 116, and since question 5 the lexer has a composed claim |
| `Text` across the host boundary | Question 13. Literals pass as two words. `Text<N>` values and the literal cast arrived in absorption 122 and agree with the virtual machine natively. Operations beyond the cast wait on question 25 |
| Opaque host handles | Question 14. Handles pass as typed host pointers on four routes and inside a struct field, a tuple, an array and an enum payload. A data slot holding one is refused by the reference compiler |
| Native WCET | Question 1. Best effort, in bytecode cost units |
| The bytecode flat-machine ISA | Question 2. Deferred |
| `RUNTIME_FAULTS.md` native defaults | Resolved. V0.2.X rewrote the specification in its pull request 598 |
| `schema_hash` and slot types | Resolved. Since absorption 119 the hash covers each slot's declared type, and native hot swap refuses a slot type change, as `native_hot_swap.rs` shows |
