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
| A local holds `Unit` after `Reset` on one path and a value on another | Since 2026-10-29, 2 of 30 corpus coroutines need a run-time kind. `14_frame_log.kel` needs a kind guard, which an empty start for private slots removes, and `interp.kel` reads an operand that may hold more than one kind. The ten `piano_roll` coroutines counted before were this line's census counting stack entries no instruction reads. See `ORDER_6_RULINGS_PLAN.md` | V0.2.X for the guard, by question 20. The cause in `interp.kel` is not yet identified | The coroutine keeps a run-time kind there, and `LowerOptions::static_kinds` refuses it |
| A composite built inside a loop and yielded from it | `13_telemetry_stream.kel` is refused by the coroutine route, since reusing the construction's fixed offset would hand the host the next iteration's bytes | V0.2.X, a language rule or a copy at the yield | Refused, not miscompiled |
| The verifier's text pass refuses a comparison of two `Word` call results | The lexer's `intern_id`, chunk 5 instruction 83. The lexer exports no composed cost claim. See `ORDER_4_COMPLETION.md` | V0.2.X, the verifier | The claim is reported as not established |
| A `Text` crossing the host boundary has no native representation | A `Text` from a call, a parameter, a native or a field is refused in a comparison, and a `Text` field is refused in a composite. See `TEXT_FIELD_VERDICT.md` and `text_comparison.rs` | The operator, a host string ABI | Literals compare by content, everything else refuses |
| Opaque host handles have no flat layout | Refused on every native route | The operator, a host reference ABI | Refused |
| Native WCET, hard or best effort | Roadmap open decision 1 | The operator | Best effort, in bytecode cost units |
| The bytecode half of the flat-machine ISA | Roadmap open decision 3 | The operator | The flat machine is native only |
| `RUNTIME_FAULTS.md` native default values | Superseded by the 2026-10-20 ruling that only `Trap` traps | V0.2.X, the specification | Native code follows the bytecode `Trap` |
| `schema_hash` covers slot names and visibility, not types | `native_hot_swap.rs` applies the same rule as `replace_module` | V0.2.X, `compute_schema_hash` | The same rule in both shapes |

Each row is also an entry in this line's open question list,
[`v0.3.0-QUESTIONS.md`](../process/handoffs/v0.3.0-QUESTIONS.md), where it
needs an operator answer.
