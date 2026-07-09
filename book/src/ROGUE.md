# Roguelike Manual

> **Navigation**: [Guide](introduction.md) | [Documentation Root](../../docs/README.md)

## Contents

1. [How this document relates to the source](#how-this-document-relates-to-the-source)
2. [What this example demonstrates](#what-this-example-demonstrates)
3. [Building and running](#building-and-running)
4. [Controls](#controls)
5. [Gameplay rules](#gameplay-rules)
6. [Host and script architecture](#host-and-script-architecture)
7. [Reading the game-tick script](#reading-the-game-tick-script)
8. [Reading the dungeon generator](#reading-the-dungeon-generator)
9. [Hot reload](#hot-reload)
10. [Reading the player AI script](#reading-the-player-ai-script)
11. [Reading the combat script](#reading-the-combat-script)
12. [Reading the artificial-intelligence archetypes](#reading-the-artificial-intelligence-archetypes)
13. [Reading the item-effect scripts](#reading-the-item-effect-scripts)
14. [Reading the bestiary script](#reading-the-bestiary-script)
15. [Reading the consume and descend scripts](#reading-the-consume-and-descend-scripts)
16. [Exercises for the reader](#exercises-for-the-reader)
17. [Capstone projects](#capstone-projects)
18. [Reference tables](#reference-tables)

## How this document relates to the source

The roguelike example splits its source across two directories. The Rust host code lives under `examples/rogue/`. The twenty-four Keleusma scripts live under `examples/scripts/rogue/`. The `include_str!` lines in `examples/rogue/main.rs` reference the script directory through a relative path, and the `SCRIPT_DIR` constant in the same file points there for the hot-reload path. This manual is the long-form companion to the example. It describes the rules of the game, the architecture of the host, the responsibilities of each script, and a graded set of exercises a reader can attempt to deepen familiarity with the embedding pattern.

The bestiary, item, and stat tables are defined inline in the host source rather than reprinted in this manual. The numbers cited in the gameplay section are stable design defaults, but the source is authoritative if they ever drift.

## What this example demonstrates

The example is built around a thin-client philosophy. The Rust host does three things and three things only: capture user input, display user output, and manage Keleusma script invocation including initialisation and native plugging. Every gameplay rule lives in a Keleusma script. The host's natives are the application programming interface boundary between display-and-input and game logic.

Seven patterns are on display.

First, a `loop main` script that drives every game tick. `rogue_game.kel` is the example's per-turn orchestrator. The host resumes it once per player input. The body applies the player command, iterates every monster, dispatches each monster's archetype, and ticks end-of-turn book keeping. See [Reading the game-tick script](#reading-the-game-tick-script).

Second, a one-shot generator script. `rogue_dungen.kel` writes the map through host natives. The script runs to completion once per floor descent. This is the natural use of `fn main` with side-effecting natives.

Third, a per-event pure function. Seven of the eight artificial-intelligence archetypes are `fn main` scripts that take a snapshot of monster and world state and return an action tuple. The script does not mutate the world directly; the host validates the returned action and commits the change. Several monster kinds share each archetype, and stat differences come from the host-side bestiary table.

Fourth, a `loop main` script that holds state across calls. The boss archetype uses the stream-chunk shape with yield. A data-segment turn counter persists across calls so the boss runs a multi-turn attack pattern. See [The boss loop main shape](#the-boss-loop-main-shape).

Fifth, host-driven dispatch with thin scripts. The item-effect scripts are tiny `match` tables mapping an effect identifier to a delta plus a status code. The host applies the deltas and executes the status action. This split keeps engine-touching code in the host and gameplay rules in the script.

Sixth, hot reload. The `F5` keybind re-reads every script from disk, recompiles, and atomically swaps the new modules into the running virtual machines. The world state survives the swap. See [Hot reload](#hot-reload).

Seventh, the player as an actor. `rogue_player_ai.kel` is shaped like every other artificial-intelligence script. The host dispatches it through the same per-actor path it uses for monsters. The player's distinction is the source of intent, not the dispatch shape. Combat math also lives in script through `rogue_combat.kel`. See [Reading the player AI script](#reading-the-player-ai-script) and [Reading the combat script](#reading-the-combat-script).

The example also demonstrates the conservative-verification discipline. Every loop in every script has a statically extractable iteration bound. Dynamic upper limits are written as fixed-bound loops with conditional bodies. Recursion is absent. The verifier accepts every shipped script.

### Known deferred items

These behaviours are intentionally left as exercises rather than shipped features. Each is documented in the [Exercises for the reader](#exercises-for-the-reader) section with the relevant entry number.

- **Sleep, Confusion, and Remove Curse scrolls** emit placeholder messages. The script-side dispatch produces the correct status codes; the host-side application is deferred. See Exercise 3.7.
- **Starvation tuning overshot.** The combination of halved hunger cadence plus corpse drops makes starvation effectively impossible. See Exercise 5.1.
- **The bestiary and gear scripts are not on the F5 hot-reload path.** Both load once at startup; modders editing those scripts must restart. The pattern admits reload but the wiring is not yet in place.
- **Weapon and armor names live in `WEAPON_NAMES` and `ARMOR_NAMES` host-side constants.** Monster names already moved into the bestiary script; the equivalent migration for the gear script is mechanical and is filed as Exercise 4.3.
- **The placeholder potion effects (Speed, Levitation, See Invisible) have script-side handlers but no host-side response.** The status codes propagate; the host treats them as no-ops with a generic message.

None of these block normal play. The game is reachable from floor one to the floor one hundred exit with the shipped configuration.

## Building and running

```bash
cargo run --release --example rogue --features sdl3-example
```

The example requires the `sdl3-example` Cargo feature, which pulls in the Simple DirectMedia Layer 3 dependency that powers the window and event loop. Static string literals used by the item-message system are unconditional in V0.2.0; the retired V0.1.x `text` cargo feature is no longer present.

The host opens a sixty-four-by-forty tile grid window. A two-row head-up display sits above the grid and a message row sits below. Each display tile is sixteen pixels square so the window is one thousand twenty-four by seven hundred and twelve pixels, which matches a sixteen-by-ten aspect ratio for the map area and fits comfortably on standard laptop displays. The procedural sprite art is authored at twenty-four pixels and downscaled to sixteen at copy time so the larger authoring size preserves the original sprite detail.

## Controls

| Key | Action |
|---|---|
| Arrow keys, `h`, `j`, `k`, `l` | Cardinal movement, one tile per press. |
| `y`, `u`, `b`, `n` | Diagonal movement. |
| Period, Space | Wait one turn in place. |
| `Q` | Quaff the held potion. The potion's effect resolves immediately and the slot empties. |
| `R` | Read the held scroll. The scroll's effect resolves immediately and the slot empties. |
| `F5` | Hot reload every Keleusma script from disk. See [Hot reload](#hot-reload). |
| Escape | Quit the example. |

There is no inventory management surface. Food eats on contact. Gold piles add to the score on contact. Weapons and armor auto-equip when an upgrade is stepped over; non-upgrade weapons and armor are destroyed on contact rather than left blocking the cell. Potions and scrolls auto-pickup when the corresponding slot is empty. If the slot is full, a message describes the ground item by its disguised name and the held item by its disguised name.

The head-up display is split across two rows. The top row is the hit-point pip strip by itself. The player's hit-point cap grows by three on every stairs descent, so at deep floors the strip can run across most of the window; giving it the whole row removes the layout collisions the prior single-row design produced at high floor counts. The second row carries, from left to right, an icon plus tier pip strip for the equipped weapon, an icon plus tier pip strip for the equipped armor, cyan depth ticks at the centre, a text readout giving the current floor number and the player's gold-as-score counter, the held potion and held scroll icons tinted by the per-run appearance colour, and amber hunger pips on the right. The tier pip strip fills one pip per gear level on a zero through nineteen scale. Bitmap-font text rendering is local to the example through `examples/rogue/text.rs`.

On death or victory the game blocks gameplay input and overlays a centred panel showing the outcome title plus final floor, gold, and turn count. Any keypress while the panel is shown exits the example.

## Gameplay rules

### Combat

- Walking into a monster initiates a melee attack.
- The hit roll is `1d20 + attacker_skill >= 10 + defender_evasion`.
- A natural one is an automatic miss. A natural twenty is an automatic hit. Critical hits double the attacker's damage input before armor is subtracted.
- Damage is `attacker_damage - defender_armor`, floored at one.
- Defender evasion for the player is the player's current level. Defender armor for the player is the equipped armor's defense value.
- Monsters with the Fast artificial-intelligence archetype act twice per turn.

### Hit points, hunger, and regeneration

- The player begins at twelve out of twelve hit points.
- Hunger starts at one hundred and ticks down by one every two turns. Food restores forty hunger. At hunger zero, the player loses one hit point per turn from starvation.
- The player regenerates one hit point every ten turns when hunger is positive and current hit points are below maximum.

### Levelling

- The player gains a level upon descending stairs to a new floor. Maximum hit points increase by three. Skill increases by one. Current hit points gain the same delta as maximum so the newly added slots come in full, but pre-existing damage persists.

### Floors and bestiary distribution

- One hundred floors. Stairs down lead deeper. Floor one hundred has an exit tile rather than stairs down.
- Each floor has a favourite monster kind. Half the spawned monsters on a floor are of the favourite kind. The other half are drawn from the pool of monster kinds the player has already encountered on previous floors, sampled with equal weight.
- Floor one has only the floor-one favourite. Floor two onward draws from the cumulative pool.

### Items

- The held potion slot and the held scroll slot each carry one item. Quaffing or reading the held item resolves the effect and empties the slot.
- Item-effect scripts decide the effect. The host applies the script's returned deltas and status action.
- Each potion and scroll has a stable per-run hidden identity. The bottle colour or the scroll's mock title shows in messages until the player first uses an item of that type. After first use, all future messages refer to the type by its true name.
- Slain monsters have a chance of leaving a corpse on the cell where they fell. The drop chance and the corpse's effect on hunger and hit points come from the bestiary entry's shape. Larger creatures yield more meat. Serpents, insects, and the mage shapes are poisonous and inflict a small hit-point penalty when eaten. Skeletons, ghosts, and slimes leave nothing behind. Stepping onto a corpse autopickups and eats it in the same turn. Players who wish to avoid a poisonous corpse should kill the offending creature in an open room rather than a corridor so they can step around the body.

### Victory and death

- Stepping onto a stairs-down tile descends automatically. Stepping onto the exit tile on floor one hundred wins the game.
- Reaching zero hit points ends the game.

If the player arrives on stairs through teleportation rather than movement, the auto-descend does not fire because teleport does not pass through the movement resolver. Stepping off and back onto the stairs triggers descent normally.

## Host and script architecture

The host owns all mutable game state. The map, the player, the monster table, the item table, the field-of-view buffers, and the random-number generator state all live in Rust. Scripts read inputs through function parameters and write outputs through return values. The few mutating natives are confined to the dungeon generator's surface.

```
+-------------------+         +-------------------+
|  examples/rogue/  |  Arc<>  |   World state     |
|     host code     | <-----> |  (map, player,    |
|  (Rust + SDL3)    |         |   monsters, ...)  |
+-------------------+         +-------------------+
        |
        | per-virtual-machine `register_native_closure` and `vm.call(...)`
        v
+------------------------------------------+
|  Twenty-four Keleusma virtual machines   |
|  - rogue_game.kel          (loop main)   |
|  - rogue_dungen.kel        (one-shot)    |
|  - rogue_player_ai.kel     (pure fn)     |
|  - rogue_combat.kel        (pure fn)     |
|  - rogue_book_keeping.kel  (pure fn)     |
|  - rogue_pickup.kel        (pure fn)     |
|  - rogue_move_resolve.kel  (pure fn)     |
|  - rogue_ai_idle.kel       (pure fn)     |
|  - rogue_ai_chaser.kel     (pure fn)     |
|  - rogue_ai_wander.kel     (uses rng)    |
|  - rogue_ai_sleeper.kel    (pure fn)     |
|  - rogue_ai_ranged.kel     (pure fn)     |
|  - rogue_ai_fast.kel       (pure fn)     |
|  - rogue_ai_smart.kel      (pure fn)     |
|  - rogue_ai_boss.kel       (loop main)   |
|  - rogue_ai_tracker.kel    (loop main)   |
|  - rogue_ai_hunter.kel     (loop main)   |
|  - rogue_item_potion.kel   (pure fn)     |
|  - rogue_item_scroll.kel   (pure fn)     |
|  - rogue_descend.kel       (pure fn)     |
|  - rogue_consume.kel       (uses natives)|
|  - rogue_scroll_apply.kel  (uses natives)|
|  - rogue_bestiary.kel      (startup load)|
|  - rogue_gear.kel          (startup load)|
+------------------------------------------+
```

The host's role is intentionally narrow. It captures user input through the Simple DirectMedia Layer 3 event pump, displays the world state through Simple DirectMedia Layer 3 rendering, and manages the virtual machines (compile sources, build the pool, register natives, drive the dispatch). Gameplay rules live in scripts. Combat math, player input interpretation, monster behaviour, item effects, and dungeon generation are all script-side. The host's natives are mostly primitive accessors and the orchestration glue that the scripts cannot replicate inside the verifier's bounds.

Each virtual machine has its own arena. The arenas live for the duration of the program. Scripts call `vm.call(...)` per invocation; the machine resets between calls.

Modules in the host source.

| Module | Responsibility |
|---|---|
| `main.rs` | Entry point, SDL3 setup, script compilation, event loop. |
| `world.rs` | Map, player, monster, item, message log, field-of-view buffers. |
| `bestiary.rs` | One hundred monster kinds organised easy-to-hard. |
| `items.rs` | Weapon, armor, potion, scroll tables. Per-run shuffled appearances. |
| `tiles.rs` | Procedural sprite atlas built from primitives on SDL3 textures. |
| `render.rs` | Head-up display, tile grid with field-of-view shading, monster and item draws, message bar. |
| `input.rs` | Keyboard-to-command translation. |
| `fov.rs` | Recursive shadowcasting on the eight octants. |
| `combat.rs` | Thin wrapper that samples the d20 roll, dispatches the combat virtual machine, and applies damage to the world. |
| `ai.rs` | Pool of artificial-intelligence, item-effect, player, and combat virtual machines. |
| `natives.rs` | Host natives the scripts call. Includes the game-tick natives (`host::run_player_turn`, `host::monster_count`, `host::run_monster_ai`, `host::tick_book_keeping`) and the dungeon-generator natives. |

## Reading the game-tick script

`rogue_game.kel` is a `loop main` script the host resumes once per player input. Each turn the body applies the player's command, iterates every monster on the floor, dispatches each monster's artificial-intelligence script, and ticks end-of-turn book keeping. The script yields one outcome code per turn and the host reads it to decide between continuing play, regenerating the next floor, ending the run with victory, or ending the run with the player's death.

```keleusma
loop main(cmd: Word) -> Word {
    let player_outcome = host::run_player_turn(cmd);
    let outcome = if player_outcome == 0 {
        let count = host::monster_count();
        for i in 0..24 {
            if i < count {
                host::run_monster_ai(i);
            };
        }
        host::tick_book_keeping()
    } else {
        player_outcome
    };
    let _ = yield outcome;
    0
}
```

This is the example's most direct demonstration of two patterns at once.

First, `loop main` with persistent state. The script is a coroutine whose body re-executes per host call. The data segment carries any state the script wants to remember across calls. The boss artificial-intelligence script demonstrates non-trivial state through a turn counter; the game-tick script keeps the data segment empty because each turn is computed afresh from the world state queried through natives.

Second, the for-each monster pattern. The script iterates every monster slot with a fixed-bound for loop and skips iterations beyond the current monster count. Inside the body, `host::run_monster_ai(i)` performs the per-monster work. The native does the heavy lifting. It looks up the monster's archetype, locks the artificial-intelligence pool, dispatches the matching virtual machine with the monster's current position and the player's position and a line-of-sight flag, releases the pool lock, and applies the returned action against the world. The script merely orchestrates the iteration.

The four natives the script consumes.

| Native | Effect |
|---|---|
| `host::run_player_turn(cmd)` | Dispatch the player artificial-intelligence script with the player's current position and the supplied keypress, then route the returned action through the same per-actor resolver that handles monster actions. Returns 0 to continue the turn, 1 if stairs were descended, 2 if the exit on floor one hundred was reached, 3 if the player died. |
| `host::monster_count()` | Number of monsters currently on the floor. |
| `host::run_monster_ai(idx)` | Dispatch the artificial-intelligence for monster `idx` and apply the returned action. Internally handles the Fast archetype's two-action turn. |
| `host::tick_book_keeping()` | Advance hunger by one on every second turn, apply starvation damage if hungry, regenerate one hit point if conditions allow, and recompute the field of view. Returns 0 if alive, 3 if the player died from starvation. |

The command codes the script and the host agree on.

| Code | Meaning |
|---|---|
| 0 | Wait |
| 1 to 8 | Move north, south, west, east, north-west, north-east, south-west, south-east |
| 9 | Descend stairs or step on exit |
| 10 | Quaff held potion |
| 11 | Read held scroll |

The fixed loop bound is twenty-four. The verifier accepts the loop because the bound is a literal. The dynamic monster count is enforced by the `if i < count` guard inside the body.

## Reading the dungeon generator

`rogue_dungen.kel` is a one-shot `fn main(floor: Word) -> Word` invoked once per floor descent. The script lays out a rooms-and-corridors map through host natives.

The high-level shape.

1. Call `host::clear_floor` to reset the map and entity lists.
2. Place eight rectangular rooms at random positions. Room dimensions are between four and nine tiles per axis.
3. Connect consecutive rooms in placement order with an L-shaped corridor between their centres. After seven corridors every room is reachable from room zero.
4. Place the player at room zero's centre.
5. Place stairs down at room seven's centre. On floor one hundred, place the exit tile instead.
6. Spawn monsters per the floor distribution. Half are the floor's favourite kind; the other half draw from the previous-floor pool.
7. Spawn items. Three to five food, two to four potions, two to four scrolls, zero to one weapon and armor upgrade, four to seven gold piles.

The chain is correctness-safe because `carve_room` only writes the interior as floor and relies on the solid wall left by `host::clear_floor` for the room's outline. Two overlapping rooms merge their floors into one connected region rather than fighting over the overlap cells. An earlier version of the carver wrote walls over the entire room rectangle before carving the interior, which would subdivide overlapping rooms and produce small unreachable pockets the chain corridor could not breach. Removing that destructive wall-fill restored connectivity at the cost of one line of code.

The verifier-driven idiom. Every loop in the script uses a fixed upper bound and a conditional body so the structural verifier accepts the iteration bound. Where the script wants a dynamic count, the loop runs to the maximum possible count and the body is guarded by `if i < count`. Room storage uses fixed-size arrays declared in the data segment because the verifier rejects dynamic growth.

The host natives the script consumes.

| Native | Effect |
|---|---|
| `host::clear_floor()` | Reset every map cell to wall, drop every monster and item. |
| `host::map_set(x, y, tile)` | Set the tile identifier at (x, y). |
| `host::map_get(x, y)` | Read the tile identifier at (x, y). |
| `host::map_w()`, `host::map_h()` | Map dimensions. |
| `host::place_player(x, y)` | Position the player. |
| `host::place_stairs(x, y)`, `host::place_exit(x, y)` | Stairs down or exit. |
| `host::spawn_monster(kind, x, y)` | Spawn from the bestiary. |
| `host::spawn_item(kind, subtype, x, y)` | Spawn into the item table. |
| `host::rng_range(lo, hi)` | Random integer in `[lo, hi)`. |
| `host::floor()` | Current floor number. |

Tile identifiers are stable. Zero is floor, one is wall, two is door closed, three is door open, four is stairs down, five is exit. The script uses these as integer literals.

## Hot reload

Pressing `F5` re-reads every Keleusma script from disk, recompiles each, and atomically replaces the running virtual machines. The world state is not touched; the player keeps current hit points, hunger, equipment, and floor. The next monster turn dispatches against the freshly reloaded artificial-intelligence scripts and the next stairs descent invokes the freshly reloaded dungeon generator.

The reload reads from the directory recorded at compile time through `concat!(env!("CARGO_MANIFEST_DIR"), "/examples/scripts/rogue")`. The initial script load uses `include_str!` so the example runs without filesystem access. Hot reload requires the script files to be present at the recorded path.

The reload is atomic. Every script is read, then every script is compiled. If any source fails to read or compile, no virtual machine is touched and the message log records the failure together with the offending script name. If every source compiles, every virtual machine is swapped at once.

This is the primary mechanic for a Keleusma-driven modding workflow. An author edits a script in another window, presses `F5` in the running game, and observes the new behaviour immediately. Examples of common workflows.

- Tune a monster's chase behaviour in `rogue_ai_chaser.kel`, save the file, press `F5`, and verify the change against the monster currently on screen.
- Add a new potion effect by editing `rogue_item_potion.kel`, then quaff the corresponding potion to test the new branch.
- Adjust the dungeon-generation parameters in `rogue_dungen.kel`, then descend stairs to generate the next floor with the new rules.

## Reading the player AI script

The example treats the player as an actor with its own artificial-intelligence script. `rogue_player_ai.kel` is symmetric in shape with the monster archetypes. The host dispatches it once per turn through the same per-actor pattern that dispatches every monster. The only difference is the source of intent. Monster intent comes from per-archetype logic. Player intent comes from the keyboard, encoded as a small integer.

```keleusma
fn main(mx: Word, my: Word, cmd: Word) -> (Word, Word, Word) {
    // cmd 0..=8: wait or move (eight directions)
    // cmd 9: descend stairs
    // cmd 10: quaff held potion
    // cmd 11: read held scroll
    // Returns (action, tx, ty) in the same shape monster archetypes use.
}
```

The host's `host::run_player_turn(cmd)` native dispatches this script, then routes the returned action through the same resolver that handles monster actions. Movement and melee flow through the same `MoveOrMelee` path. Player-only actions (descend, quaff, read) are additional action codes that the monster archetypes never emit.

The benefit of this symmetry is conceptual rather than mechanical. A reader who understands how a monster's turn works automatically understands how the player's turn works.

## Reading the combat script

`rogue_combat.kel` holds the hit and damage rules. The host samples the d20 roll from its random number generator and dispatches the combat virtual machine with attacker skill, attacker damage, defender evasion, defender armor, and the roll. The script returns `(hit_kind, damage)` where `hit_kind` is zero for a miss, one for an ordinary hit, or two for a critical hit.

```keleusma
fn main(
    attacker_skill: Word,
    attacker_damage: Word,
    defender_evasion: Word,
    defender_armor: Word,
    roll: Word,
) -> (Word, Word)
```

Combat math is now a single Keleusma file that a reader can rebalance without touching the host. The Rust side of combat is the thin wrapper in `examples/rogue/combat.rs`, which samples the roll, dispatches the script, applies the returned damage to the world, and posts the message.

## Reading the artificial-intelligence archetypes

Each monster kind in the bestiary names an artificial-intelligence archetype. The host instantiates one Keleusma virtual machine per archetype. Per monster turn, the host invokes the archetype's virtual machine with the monster's position, the player's position, and a line-of-sight flag, and reads back an action tuple.

Call convention.

```keleusma
fn main(mx: Word, my: Word, px: Word, py: Word, sees_player: Word)
    -> (Word, Word, Word)
```

Return tuple `(action, tx, ty)`.

| Action | Meaning |
|---|---|
| 0 | Wait. (tx, ty) ignored. |
| 1 | Move or melee into cell (tx, ty). The host applies the move if walkable, resolves a melee attack if the cell is the player's, and rejects non-adjacent targets. |
| 2 | Ranged attack at cell (tx, ty). The host applies the attack only if (tx, ty) is the player's cell. |

The eight archetypes shipped with the example.

| Script | Behaviour |
|---|---|
| `rogue_ai_idle.kel` | Waits. Used by stationary creatures. |
| `rogue_ai_chaser.kel` | Greedy one-step chase when the player is visible. |
| `rogue_ai_wander.kel` | Random cardinal step when the player is not visible. Greedy chase when visible. |
| `rogue_ai_sleeper.kel` | Waits until the player enters line of sight, then chases. |
| `rogue_ai_ranged.kel` | Retreats when adjacent. Ranged attack when in sight and out of melee. |
| `rogue_ai_fast.kel` | Behaves like the chaser. The host double-invokes the script per turn. |
| `rogue_ai_smart.kel` | Heuristic step on the dominant axis when the player is visible. |
| `rogue_ai_boss.kel` | Four-turn attack pattern alternating ranged and chase. Implemented as `loop main` with a turn counter in the data segment so the phase persists across calls. Currently bound only to the Balrog Lord on floor one hundred. See [The boss loop main shape](#the-boss-loop-main-shape). |
| `rogue_ai_tracker.kel` | Remembers the player's last seen position in the data segment. Chases directly when the player is visible; moves toward the remembered cell when out of sight. Implemented as `loop main` with a three-slot data segment. Bound to wraiths, specters, vampire spawns, mind flayers, and the bone devil. |

The wander script uses `host::rng_range` for the random direction. Every other archetype except the boss is a pure function of its inputs.

The archetypes are intentionally minimal so the patterns are easy to read. A reader who wants more elaborate behaviour writes a new script and assigns it to a bestiary entry through the `AiKind` enumeration. The exercises section below works through this case.

### The boss loop main shape

The Balrog Lord's artificial intelligence is the example's demonstration of `loop main`. Where every other archetype is a one-shot `fn main`, the boss script declares a data segment with a turn counter and yields one action per host call.

```keleusma
data state {
    turn: Word,
}

loop main(input: (Word, Word, Word, Word, Word)) -> (Word, Word, Word) {
    // Body computes the action.
    // ...
    state.turn = state.turn + 1;
    let _ = yield action;
    (0, 0, 0)
}
```

The five-tuple input packs the same five fields the other archetypes receive as separate arguments. Packing into a tuple is necessary because `vm.resume` accepts exactly one `Value`. The body executes the standard ranged-then-chase decision based on `state.turn % 4`, increments the counter, and yields the action.

The host dispatches the boss differently from the other archetypes. The first turn calls `vm.call(...)`; every subsequent turn calls `vm.resume(...)`. Each call or resume runs the body once from its current position. On reaching the trailing `(0, 0, 0)` expression after yield, the virtual machine emits `VmState::Reset`. The host walks past `Reset` and calls `vm.resume` again to drive the body through its next iteration. The result is one yielded action per host-driven logical turn.

The data segment persists across calls so `state.turn` increments monotonically. Hot reload resets the data segment, so the four-turn pattern restarts from phase zero whenever the script is replaced.

Future archetypes with multi-turn behaviour follow the same pattern. A guardian that patrols between waypoints, a necromancer that periodically spawns minions, or a hydra that grows new heads as it takes damage all fit naturally into `loop main` with state.

### Restarting a loop main script after game-over

The same Reset semantics that drive the per-turn cadence also give the example a clean replay path on game-over. The host does not rebuild the boss, tracker, or hunter virtual machines when the player presses R to start a new run. It zeroes their data segments and leaves the virtual machines parked at their current yield point. On the next dispatch the body resumes, observes the freshly reset state, and produces the first action of a new run identical in shape to the first action of the original run. The wrap from the trailing tuple back to the top of the body happens through the existing Reset boundary that already separates one turn from the next.

The data-segment reset is the only step required because the body re-reads the world through natives on every iteration. Persistent state lives in two places only. The bestiary entry is host-owned and identical between runs. The data segment is per-virtual-machine and is what the reset clears. The body's local variables do not persist past a yield because the stack frame is the activation record for a single body execution rather than the script as a whole. Combined with a fresh `World` and a new dungeon, the next call against a reset virtual machine therefore produces a clean replay equivalent to the first run.

The implementation is `AiPool::reset_loop_main_data` in the example's `ai.rs`. It walks the three slot-zeroing pointers into the boss, tracker, and hunter data segments and writes zero across each one. No equivalent reset is needed for the per-monster `fn main` archetypes because they carry no data segment.

## Reading the item-effect scripts

`rogue_item_potion.kel` and `rogue_item_scroll.kel` are one-shot `fn main` scripts the host invokes when the player quaffs or reads the held item. The scripts dispatch on the effect identifier and return a five-element tuple `(hp_delta, max_hp_delta, skill_delta, status_code, status_arg)`. The host applies the deltas to the player and executes the status action.

Status codes.

| Code | Action |
|---|---|
| 0 | None. |
| 1 | Magic mapping. Reveal the entire floor as explored. |
| 2 | Teleport. Warp the player to a random walkable cell. |
| 3 | Identify. Mark every potion as identified. |
| 4 | Enchant weapon. Advance the equipped weapon tier by `status_arg`. |
| 5 | Enchant armor. Advance the equipped armor tier by `status_arg`. |
| 6 | Light. Mark a small radius around the player as explored. |
| 7 | Detect monsters. Post the floor's monster count to the message log. |
| 8 | Sleep. Placeholder, no effect. |
| 9 | Confusion. Placeholder, no effect. |
| 10 | Remove curse. Placeholder, no effect. |
| 11 | Restoration. Heal the player to full hit points. |

The split between effect logic in script and status action in host is deliberate. Scripts describe what the effect produces. The host applies the engine-specific changes. Effects with delta-only behaviour stay in script. Effects that touch the field-of-view buffers, the random-number generator, or the equipment tables route through the status-action mechanism.

The status-action mechanism itself runs in a second script. `rogue_scroll_apply.kel` takes the `(status_code, status_arg)` pair and dispatches to one of eight fine-grained natives: `host::set_explored_all`, `host::set_explored_radius`, `host::teleport_player_random`, `host::identify_all_potions`, `host::change_weapon_tier`, `host::change_armor_tier`, `host::sense_monsters`, and `host::scroll_placeholder`. Each native applies its world mutation and pushes its message. The split means modders can change which status code triggers which native, add new status codes, or compose multiple natives per status code, all by editing the script. The host's natives stay small and orthogonal.

## Reading the bestiary script

`rogue_bestiary.kel` is the data half of the monster system. The host runs this script once per monster id at startup and reads the resolved values from the script's data segment. The result is cached behind a `OnceLock` in `examples/rogue/bestiary.rs` so that runtime accesses through `bestiary::kind(idx)` are plain reads against a `Vec<MonsterKind>`.

### The data-loader pattern

The bestiary script is a worked example of an idiom that this example demonstrates for Keleusma. The pattern is documented in detail in the [Keleusma Cookbook's data-loader recipe](./COOKBOOK.md#the-data-loader-pattern). A short summary follows; the cookbook covers minimal examples, variations, and when to reach for the pattern.

The pattern composes three techniques. Each is individually known. The combination fits Keleusma's specific constraints (no module-scope constants, fixed-size data segment, bounded execution) particularly well, and the composition is what makes it idiomatic here.

First, **multi-headed function dispatch encodes the constant table**. Keleusma has no module-scope `const` for arrays of records, but the verifier accepts multi-headed function definitions with integer patterns. One head per entry, each body assigning the entry's fields, is functionally equivalent to a constant array. The encoding is verifier-friendly because every body is straight-line code; the dispatch itself compiles to a jump table when the integer keys are dense.

Second, **the data segment serves as the host-script I/O struct**. The data segment is normally used for state that the script preserves across `loop main` resumes. Here it carries the output fields of a one-shot pure function. The host lends a shared buffer through `vm.call_with_shared`, writes the input (the entry index) as the call argument, and reads the outputs (the entry's fields) out of the buffer through `vm.get_shared` after the call returns. No language change is required because the shared buffer and `get_shared`/`set_shared` are part of the host boundary; the repurposing is on the script side.

Third, **the negative-index convention discovers the table size**. Calling `fn main(-1)` resolves the index to `MONSTER_COUNT - 1` inside the script, writes the last entry's fields into the data segment, and returns. The host reads the `id` slot to learn the table size with one call. The host can therefore size its cache from the script rather than from a parallel host-side constant, with an assertion catching any drift.

The pattern is well-suited to other tables that share the shape "small fixed-size struct of integers, indexed by ordinal". The bestiary's per-shape corpse stats also use this pattern: the `fn corpse_fill(N)` dispatcher inside the same script reads the shape ordinal from a slot the `fn fill(N)` step has already written, then fills three more slots from a twelve-entry table.

### Worked example

The data segment declares one field per output column. The fields the `fill(N)` heads set directly (id, shape, three primary colour channels, three accent colour channels, five combat stats, ai archetype, first floor, score). The fields the `corpse_fill(shape)` step sets indirectly (drop chance, satiation, hit-point delta). One hundred multi-headed `fill(N)` functions write the per-entry constants. Twelve multi-headed `corpse_fill(N)` functions write the per-shape corpse stats. The `fn main(n)` entry point resolves negative indices to `MONSTER_COUNT + n`, calls `fill(i)`, then chains into `corpse_fill(state.shape)`.

```keleusma
data state {
    id: Word,
    shape: Word,
    primary_r: Word, primary_g: Word, primary_b: Word,
    accent_r: Word, accent_g: Word, accent_b: Word,
    max_hp: Word, skill: Word, evasion: Word, damage: Word, armor: Word,
    ai: Word,
    first_floor: Word,
    score: Word,
    corpse_drop_chance: Word,
    corpse_satiation: Word,
    corpse_hp_delta: Word,
}

fn main(n: Word) -> Word {
    let count = 100;
    let i = if n < 0 { count + n } else { n };
    fill(i);
    corpse_fill(state.shape);
    0
}

fn fill(0) -> Word {
    state.id = 0; state.shape = 0;
    state.primary_r = 120; state.primary_g = 90; state.primary_b = 60;
    state.accent_r = 60; state.accent_g = 40; state.accent_b = 30;
    state.max_hp = 3; state.skill = 0; state.evasion = 1;
    state.damage = 1; state.armor = 0;
    state.ai = 2; state.first_floor = 1; state.score = 1;
    0
}
// ninety-nine more fill heads
fn fill(_n: Word) -> Word { 0 }

fn corpse_fill(0) -> Word {
    state.corpse_drop_chance = 50; state.corpse_satiation = 8; state.corpse_hp_delta = 0;
    0
}  // Tiny
// eleven more corpse_fill heads
fn corpse_fill(_n: Word) -> Word { 0 }
```

Monster names live in the script too. A third multi-headed dispatcher, `fn name(N) -> Text`, returns each entry's name as a static string. The bestiary script's `fn main` returns `name(i)` as its last expression, so the host receives the name as the call's `Finished(StaticStr(...))` payload while the data segment carries the numeric fields. The host leaks the returned string once at startup to obtain a `&'static str` for caching. Keleusma's data segment does not currently accept string fields in source, but a function return is a clean alternative and admits the no-host-side-name-table outcome. The host's `MONSTER_COUNT` constant mirrors the script's `count` literal; the startup assertion catches any drift between the two.

The shipped script weighs in at roughly three hundred and eighty lines for the three dispatchers combined (one hundred `fill` heads, twelve `corpse_fill` heads, one hundred `name` heads). The prior Rust `MonsterKind` struct-literal form took fourteen lines per entry plus a parallel hundred-line name array plus three twelve-arm corpse methods. The bestiary migration is a clear net reduction concentrated where the per-entry density mattered most.

## Reading the consume and descend scripts

Two short scripts wrap recurring world mutations so the host's autopickup driver and stairs-descent path can stay narrow.

`rogue_consume.kel` is the per-kind consumption table. After `rogue_pickup.kel` returns `consume`, the host invokes `rogue_consume.kel` with the item kind and subtype. The script dispatches to one of seven fine-grained natives: `host::consume_food`, `host::take_gold`, `host::equip_weapon`, `host::equip_armor`, `host::stash_potion`, `host::stash_scroll`, and `host::eat_corpse`. Each native applies the matching world mutation and pushes the matching message. Adding a new item kind takes one new native plus one new arm in the script.

`rogue_descend.kel` is the per-floor level-up calculator. The host snapshots the player's current level, hit-point cap, hit points, and skill plus the current floor; the script returns the post-descent five-tuple. The shipped script adds three to the hit-point cap and three to current hit points, adds one to skill, and increments level by one. Modders can change the progression curve or split the increment across stats without touching the host.

## Exercises for the reader

The exercises below are graded by depth. Tier one exercises change values in existing scripts or tables. Tier two exercises introduce new content that fits the existing dispatch shape. Tier three exercises change the dispatch shape itself.

Each exercise lists what to change, where to change it, and a verification suggestion. Most are answerable without writing tests; readers who want to be thorough can extend `tests/rogue_scripts.rs` to cover their additions.

### Tier one: parameter tuning

**Exercise 1.1.** Raise the player's starting hit points from twelve to twenty. Locate the change site in `examples/rogue/world.rs` and start a fresh run to confirm the head-up display draws twenty pips. Hypothesis to verify. The hit-point gauge layout still fits because the head-up display row spans the full window width.

**Exercise 1.2.** Tune the hunger cadence. The shipped configuration ticks hunger down by one every two turns. Find the tick site in `examples/scripts/rogue/rogue_book_keeping.kel` and try faster (every turn) or slower (every three turns) cadences. Observe the run-length effect at floor counts of ten, twenty, and fifty. Hypothesis. Faster cadence forces aggressive monster killing for corpses; slower cadence makes hunger irrelevant. Pick the cadence that produces the most interesting decisions about when to fight, when to flee, and when to eat a poisonous corpse.

**Exercise 1.3.** Add a ninth weapon tier called "soulrender" with damage forty-two. The weapons table lives in `examples/rogue/items.rs`. Confirm that the dungeon generator's tier-clamp expression still places it correctly on the deepest floors.

**Exercise 1.4.** Make the Sewer Rat's first appearance start with five hit points instead of three. The bestiary table is in `examples/rogue/bestiary.rs`. Run a fresh game and observe how many hits the player takes to fell a rat.

**Exercise 1.5.** Change the room count in the dungeon generator from eight to twelve. The room storage in `rogue_dungen.kel` uses a fixed-size array. The array declaration must change in lockstep with the literal in the `for i in 0..8` loops. Inference. The verifier rejects the script if the array bound and the loop literal disagree.

### Tier two: shaped additions

**Exercise 2.1.** Add a new artificial-intelligence archetype called Coward that chases the player when above half hit points and flees when below. The script takes the same five inputs as every other archetype but returns the move-toward-player action when `monster_hp > monster_max_hp / 2` and the move-away action otherwise. The script does not currently receive monster hit points, so the exercise has two parts. First, extend the call convention to accept hit points as additional parameters. Second, write the script. The host call sites that need adjustment are in `examples/rogue/natives.rs::run_one_monster_turn` and `examples/rogue/ai.rs::AiPool::dispatch`. Inference. Adding parameters affects every archetype script, so the call convention change is the larger commitment.

**Exercise 2.2.** Add a new potion effect called "agility" that increases the player's evasion by one. The status code system already supports this if you introduce code twelve and apply it in the host. The script change is in `rogue_item_potion.kel`. The host change is in `examples/rogue/natives.rs::apply_potion_status`. The player has no dedicated evasion stat; you will need to add one to `examples/rogue/world.rs::Player` and reflect it in `examples/scripts/rogue/rogue_combat.kel`.

**Exercise 2.3.** Add doors to the dungeon generator. When a corridor crosses a room wall, the tile at the crossing should be tile identifier two (closed door) rather than zero (floor). The corridor carving helpers in `rogue_dungen.kel` currently write floor over wall unconditionally. The exercise is to detect the wall-crossing case and write a door instead. The host already renders both open and closed door sprites.

**Exercise 2.4.** Add a "rangedeyes" status code that reveals every monster on the floor for the next twenty turns. The detect-monsters scroll currently returns a single message about the floor's monster count. A real reveal-all-monsters effect would set a host-side timer and render every monster regardless of field of view while the timer counts down. The host work is in `examples/rogue/world.rs` for the timer field and in `examples/rogue/render.rs` for the conditional monster draw.

**Exercise 2.5.** Add a vault generator that occasionally replaces the standard room-and-corridor layout with a prefab room laid out symmetrically with treasure in the centre. The vault should appear with probability one in five on floors beyond ten. The exercise is to write a `vault_floor(floor)` branch in `rogue_dungen.kel` and gate it on the floor and a random draw.

**Exercise 2.6.** Add background music. The `examples/piano_roll.rs` example already demonstrates the full SDL3 audio pipeline against a Keleusma `loop main` score sequencer. Its design opens the SDL3 audio device, shares an eight-voice array under `Arc<Mutex<_>>` with an SDL3 audio callback, renders square-wave and triangle-wave samples on the audio thread, and drives note triggers from a Keleusma script at sixteenth-note ticks. The exercise is to adapt that pattern to the rogue host. The source-code hook is the `add audio processing here` comment in `examples/rogue/main.rs` at the SDL3 init site. The script work is a new `rogue_music.kel` `loop main` script that yields note triggers per tick. The host work is opening the audio device, spawning the audio thread, and resuming `rogue_music.kel` at a chosen cadence (per turn, per second, or per floor descent for new-floor stings). Concerns to address. The audio thread and the game tick run independently; the score should be sampled by the audio thread without blocking the game thread. The piano roll's mutex-guarded voice array works for this. Hypothesis. A slow bassline that shifts with floor depth would carry atmosphere without distracting from the silence between player inputs that this turn-based example currently relies on.

### Tier three: dispatch and architectural changes

**Exercise 3.1.** Replace the shadowcasting field-of-view algorithm with a symmetric ray-casting implementation that traces from the player to every cell within radius. Compare the visible footprint between the two algorithms on the same dungeon. The algorithmic substitution lives in `examples/rogue/fov.rs`. Inference about the trade. Shadowcasting is faster and handles thin walls cleanly. Ray casting is simpler to read and easier to make symmetric for the monster-sees-player check the host already performs separately.

**Exercise 3.2.** Add a save-and-load surface. The world record is `Serialize`-able by writing one. The exercise is to dump the entire world to a file on a dedicated keybind and load it back on next run. The Keleusma scripts do not need to change. The host work is in `examples/rogue/main.rs` and `examples/rogue/world.rs`. Unaddressed concern. The arena state is not part of the world. Scripts that hold per-call state in the data segment would lose it on reload. The dungen, artificial-intelligence, and item scripts shipped with the example are stateless across calls, so the concern is theoretical for the current roster.

**Exercise 3.3.** Replace the host-driven monster turn loop with a per-turn driver script. The current architecture has the host iterate monsters and dispatch their archetypes. An alternative is a `rogue_game.kel` script that the host calls once per turn with the world state as input and the host commits the script's emitted commands. This exercise is the largest in the manual because every host native that touches the world needs to accept calls from the new script. The pay-off is that the entire turn-loop sequencing becomes user-editable code.

**Exercise 3.4.** Replace the procedural sprite atlas with a sprite sheet loaded from a Portable Network Graphics file. The host already builds textures through SDL3 surfaces. Loading a Portable Network Graphics file and slicing it into per-tile sub-textures is mechanical work in `examples/rogue/tiles.rs`. The exercise's actual challenge is choosing the sprite sheet. The bestiary has one hundred entries.

**Exercise 3.5.** Add ranged attacks for the player. The current rules give the player melee only. Adding ranged means a wand or bow item type, an aimed-attack keybind, line-of-sight resolution from the player to the target cell, and a damage formula that accounts for distance. The host work is in `examples/rogue/natives.rs` to recognise the new action code and route it through the resolver; the script work is in `examples/scripts/rogue/rogue_player_ai.kel` to emit the ranged action and in `examples/scripts/rogue/rogue_combat.kel` to apply the distance falloff. Hypothesis. The bestiary's ranged-archetype monsters become disproportionately easy if the player can return fire from outside their attack envelope, so the damage formula may need a distance falloff to keep balance.

**Exercise 3.6.** Replace the symmetric monster line-of-sight rule with a per-monster shadowcast. The current implementation in `examples/rogue/natives.rs::monster_sees_player` treats the player's field-of-view bitmap as the ground truth: if the player can see the monster, the monster can see the player. This is symmetric by construction but ties monster perception to the player's vantage. An independent per-monster cast originating at the monster's cell with the same eight-tile radius would produce different results in pillar-like wall configurations. Implement the cast and compare. Hypothesis. The independent cast feels more "fair" but is more expensive; at the current scale the cost is negligible.

**Exercise 3.7.** Implement the placeholder potion and scroll effects. The status-code dispatch infrastructure is already in place. The pieces that remain.

- Potion of Speed (effect 7). Grant the player extra turns. Add an `extra_turns` counter to the player state. Each tick, if positive, decrement and run the player turn before yielding. Hypothesis. The player effectively moves twice; cost is balanced because the potion is consumed.
- Potion of Levitation (effect 8). Add a `levitating` timer to the player state. While positive, ignore traps (when traps land in a future revision) and treat the cell as walkable for the chamber-of-pits use case. Currently no traps exist, so the effect is a no-op against current content but reads correctly in messages.
- Potion of See Invisible (effect 9). Add an `invisible` flag to monster kinds and to instances. While See Invisible is active, the renderer shows invisible monsters as if they were visible. Requires the bestiary to gain an invisible flag and a tier of monsters that uses it.
- Scroll of Sleep (effect 8 status code). Add a `sleeping_turns` field to each monster. While positive, the host's per-monster dispatch returns Wait immediately and decrements the counter without invoking the archetype's virtual machine. Effective range two on read.
- Scroll of Confusion (effect 9 status code). Add a `confused_turns` field to each monster. While positive, the host scrambles the artificial-intelligence-returned action: with probability fifty per cent the action is rerolled as a random adjacent step.
- Scroll of Remove Curse (status code 10). Add a curse flag to weapons and armor. Some weapons spawn cursed; cursed gear cannot be unequipped. Remove Curse lifts the flag.

Each effect is a few lines on the host side. Reuse the existing status code values so the script side does not need to change.

### Tier four: research-flavoured questions

**Exercise 4.1.** What is the smallest set of artificial-intelligence archetype scripts that still produces a recognisable rogue feel across the one-hundred-floor descent? The current set is nine. Try removing one at a time and assess whether the gameplay degrades noticeably. Inference. The boss archetype is the most replaceable because it is bound to a single monster.

**Exercise 4.2.** What is the worst-case execution time of the dungeon generator on a single floor? The structural verifier accepts the script, but a measured profile would identify the dominant cost. Reading the script's per-loop body and counting host-native calls is a fair starting point. The arena-bounded text-size analysis already proves the worst-case memory usage is bounded; this exercise asks for the time companion.

**Exercise 4.3.** The bestiary now lives entirely in `rogue_bestiary.kel`, including the monster names; see the *Reading the bestiary script* section above. The names flow through the script's return value rather than the data segment. The remaining open question is the weapon and armor names in `rogue_gear.kel`. Apply the same return-value-as-name pattern to the gear script and remove `WEAPON_NAMES` and `ARMOR_NAMES` from `examples/rogue/items.rs`. The migration is mechanical given the bestiary precedent.

**Exercise 4.4.** What is the right division of responsibility between the host and the scripts in a Keleusma example? The piano-roll example puts almost everything in scripts; the roguelike example splits roughly in half by line count. Write a short essay arguing for one extreme or the other across both examples.

### Tier five: game balance

**Exercise 5.1.** Tune starvation back to a real threat. The first playtest passes ran out of food before floor four, which felt unfair. The shipped fix combined two changes. Hunger now ticks down by one every two turns rather than every turn, doubling the run length under a single starting ration. Slain monsters now have a shape-derived chance of leaving a corpse the player can step onto and eat. The combination overshot. Starvation is no longer a real threat at all; corpse pickups in particular keep the player fed indefinitely past the first few combats. The exercise is to find a configuration that puts the player back in the danger zone without making early-game starvation inevitable. Candidates to tune. The hunger cadence in `examples/scripts/rogue/rogue_book_keeping.kel`. The food restoration amount in `examples/rogue/natives.rs::autopickup`. The food spawn count in `examples/scripts/rogue/rogue_dungen.kel::spawn_items`. The corpse drop probabilities and per-shape satiation in `examples/rogue/bestiary.rs`. The art of game balance is finding the combination that produces interesting decisions about when to eat a poisonous corpse, when to skip a fight to conserve hunger ticks, and when to push for the next floor rather than searching for food.

**Exercise 5.2.** Calibrate the floor difficulty curve. Play five fresh runs and record how many turns and how many hit points the player loses on each floor up to floor twenty. Hypothesis to test. The difficulty should rise smoothly. In practice the curve has visible discontinuities at the floor boundaries that introduce a new shape (around floor twelve when serpents appear, around floor thirty-six when dragons appear). The exercise is to smooth the curve by retiming the bestiary's `first_floor` fields and the dungeon generator's monster-count scaling. Inference. A smooth curve gives the player a sense of growing power; a spiky curve produces frustration on the spike floors.

**Exercise 5.3.** Audit the loot distribution. Count how many of each item kind the player encounters in a five-floor run. The shipped configuration tends to produce more gold piles than potions and scrolls combined. Gold is purely a score counter in the current rules, so a flood of gold drops feels meaningless. The exercise is to retune `spawn_items` in the dungeon generator so the player encounters roughly equal counts of food, potions, scrolls, and gold piles. Hypothesis. Equal counts make every pickup feel deliberate; lopsided counts make rare drops feel disproportionately exciting. Pick the configuration that fits the desired play feel.

**Exercise 5.4.** Combat damage scaling. The shipped weapon table goes from two damage at tier zero to one hundred and eighteen damage at tier nineteen, and the armor table goes from zero to forty defense. The bestiary's monster hit points range from two for vermin to two hundred for the boss. The exercise is to play a run and identify the floor at which the player's current weapon stops being satisfying. Adjust either the weapon damage progression or the monster hit-point scaling so that floors thirty to fifty feel as decisive as floors one to ten. Hypothesis. The decisive feeling comes from kills per attack rather than absolute damage; a weapon that one-shots half the bestiary on its floor produces a different rhythm than one that requires three swings per kill.

## Capstone projects

These projects each take several days of focused work. They synthesise multiple tier-three exercises into a complete feature.

**Capstone A. The Ranged Combat Update.** Implement ranged attacks for the player (Exercise 3.5), add a bow item type with a quiver of arrows tracked on the head-up display, add a fire-bow keybind, and rebalance the floor twenty through forty bestiary entries so the new combat option is meaningful but not dominant. Deliverable. A playable run from floor one to floor twenty that uses ranged attacks at least ten times.

**Capstone B. Per-Run Persistence.** Implement save-and-load (Exercise 3.2), add an autosave at every stairs descent, and add a high-score table that survives game-over. Deliverable. A polished run that can be exited mid-floor and resumed.

**Capstone C. The Asset-Driven Renderer.** Replace the procedural sprite atlas (Exercise 3.4), add four-direction-aware sprites for humanoid monsters and the player, and add per-tile animation frames for water, lava, and the exit tile on floor one hundred. Deliverable. A rendered run that visibly differs from the procedural-sprite version.

**Capstone D. The Quest System.** Add a quest-board native that the host queries at floor entry. Each floor's quest is a string and a completion condition (kill ten of a kind, collect a kind of item, reach a particular tile). Add a quest-tracking field to the player struct. Add a small reward on completion. Deliverable. A run where the floor-three quest is visibly tracked and completed.

## Reference tables

### Tile identifiers

| ID | Tile |
|---|---|
| 0 | Floor |
| 1 | Wall |
| 2 | Closed door |
| 3 | Open door |
| 4 | Stairs down |
| 5 | Exit |

### Item kind identifiers

| ID | Kind |
|---|---|
| 0 | Food |
| 1 | Gold |
| 2 | Weapon |
| 3 | Armor |
| 4 | Potion |
| 5 | Scroll |
| 6 | Corpse |

### Artificial-intelligence action codes

| Code | Action |
|---|---|
| 0 | Wait |
| 1 | Move or melee into target cell |
| 2 | Ranged attack at target cell |

### Item-effect status codes

See [Reading the item-effect scripts](#reading-the-item-effect-scripts) above for the full table.

### Default tuning parameters

| Parameter | Default |
|---|---|
| Map size | Sixty-four by forty tiles |
| Display tile size | Sixteen by sixteen pixels. Sprite art is authored at twenty-four pixels and downscaled to sixteen on copy. |
| Field-of-view radius | Eight tiles |
| Starting hit points | Twelve |
| Starting hunger | One hundred |
| Hunger tick | Minus one every two turns |
| Food restoration | Forty hunger |
| Hit-point regeneration | One per ten turns when hunger is positive |
| Starvation damage | One per turn at hunger zero |
| Hit-point gain per level | Three |
| Skill gain per level | One |
| Rooms per floor | Eight |
| Monsters per floor | Floor plus four, capped at twelve |
| Food per floor | Three to five |
| Potions per floor | Two to four |
| Scrolls per floor | Two to four |
| Weapons per floor | Zero to one |
| Armor pieces per floor | Zero to one |
| Gold piles per floor | Four to seven |
