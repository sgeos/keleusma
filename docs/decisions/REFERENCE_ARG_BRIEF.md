# BRIEF — ten exempt modules, and a decline resting on the wrong comparison

## The goal

**Ten of the twenty-two exempt modules are the `piano_roll` family**, all exempt for one reason:

> *a native receives a REFERENCE argument (a string); it is an arena handle on the VM side and a
> pointer natively, so the two do not render as the same integer*

That is the single largest exempt class, and it is a **harness limit, not a backend refusal**. The
modules run fine on both sides; this harness declines to compare them.

## Why the decline was framed too widely

The handoff records three measured reasons for not closing this, all of which are about
**dereferencing the pointer** to compare string CONTENT: the module does not record native parameter
types, the native side runs in-process so a wrong pointer assumption is a segfault, and the payoff
is tidying because a hand-written differential already covers those modules.

**Every one of those objections is about the ambitious version.** None of them argues against the
cheap version, which nobody considered: **do not compare the reference argument at all, and compare
everything else.**

The virtual-machine stub ALREADY substitutes `0` for a non-scalar argument. Only the native stub
logs the raw pointer. So the two logs differ in exactly one field, and the whole module is discarded
over it — while the call sequence, the native names, every scalar argument, the return value and the
shared segment go uncompared.

**An exemption says "nothing was compared". A masked comparison says "everything except this was
compared".** The second is strictly more coverage and needs no unsafe decode.

## The mechanism, and why it is sound rather than a guess

`run_vm` runs before `run_native` for each seed — verified in the source, not assumed. The virtual
machine sees `Value` variants and therefore knows exactly WHICH ARGUMENT POSITIONS are non-scalar.
Recording those positions during the VM run and masking the same positions in the native log
compares like with like.

**State the weakening honestly.** This is a RUNTIME mask, not a contract: it rests on the VM's
observation of a particular run rather than on anything the module declares. A native side passing a
wrong pointer would be masked and not noticed — but it is not noticed today either, because nothing
is compared at all.

## Prior failures and specific wrong turns to avoid

- **Do not dereference the pointer.** The native side runs in-process through the JIT, so a wrong
  assumption is a SEGFAULT rather than a failed assertion. The string ABI is ruled PROVISIONAL, and
  building an unsafe decoder against a provisional layout is the trade already declined twice.
- **Do not mask more than the reference positions.** Masking a scalar argument would hide a real
  disagreement. The mask must be keyed to the exact `(native, position)` pairs the VM observed.
- **Check the exempt count actually moves.** If the modules stay exempt, they are exempt for a
  reason other than the one assumed — which is what happened when a corpus split failed to make a
  module executable and an external-native registration turned out to be the real blocker.
- **A newly-compared module must AGREE.** If one disagrees, that is a finding and the mask must not
  be widened to bury it. Report it.
- **Do not claim the string is verified.** It is explicitly NOT compared. Anything the handoff says
  about these modules must say what is masked, at the place the number is read.

## What a good outcome looks like

The `piano_roll` family is compared on everything except its reference arguments and agrees; the
exempt count falls by roughly ten; and the report states plainly which field is masked and that this
is a runtime mask rather than a contract.

**If a module disagrees once unmasked-except-the-pointer, that is the better outcome** — it would be
a real defect that ten modules' worth of exemption has been hiding.
