//! Artificial-intelligence and item-effect virtual-machine pool.
//! One Keleusma virtual machine per archetype. The host invokes
//! the matching virtual machine per monster per turn.
//!
//! The pool owns its arenas by leaking them at construction so
//! the contained virtual machines have `'static` arena
//! references. This allows the pool to be wrapped in
//! `Arc<Mutex<AiPool>>` and shared with native closures
//! registered against the game-tick virtual machine, which the
//! game-tick script uses to drive per-monster artificial-
//! intelligence dispatch.
//!
//! The leak is a one-time, fixed allocation per program run.

use std::sync::{Arc, Mutex};

extern crate alloc;

use keleusma::bytecode::Value;
use keleusma::compiler::compile;
use keleusma::lexer::tokenize;
use keleusma::parser::parse;
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState};
use keleusma::{Arena, Module};

/// Five-component outcome of a level-descend dispatch: the updated
/// level, max HP, HP, skill, and floor. Type alias because clippy's
/// `type_complexity` lint trips on the inline tuple at the call site
/// under `--all-features`.
type DescendOutputs = (i64, i64, i64, i64, i64);

use crate::bestiary::AiKind;

/// Embedded script sources, keyed by filename. The startup path
/// looks scripts up here; the hot-reload path reads the same
/// filenames from disk. Adding a new script means adding one
/// row to this table and one field to [`AiModules`] (or
/// referencing it directly for the standalone dungen and game
/// scripts).
pub const EMBEDDED: &[(&str, &str)] = &[
    (
        "rogue_dungen.kel",
        include_str!("../scripts/rogue/rogue_dungen.kel"),
    ),
    (
        "rogue_ai_idle.kel",
        include_str!("../scripts/rogue/rogue_ai_idle.kel"),
    ),
    (
        "rogue_ai_chaser.kel",
        include_str!("../scripts/rogue/rogue_ai_chaser.kel"),
    ),
    (
        "rogue_ai_wander.kel",
        include_str!("../scripts/rogue/rogue_ai_wander.kel"),
    ),
    (
        "rogue_ai_sleeper.kel",
        include_str!("../scripts/rogue/rogue_ai_sleeper.kel"),
    ),
    (
        "rogue_ai_ranged.kel",
        include_str!("../scripts/rogue/rogue_ai_ranged.kel"),
    ),
    (
        "rogue_ai_fast.kel",
        include_str!("../scripts/rogue/rogue_ai_fast.kel"),
    ),
    (
        "rogue_ai_smart.kel",
        include_str!("../scripts/rogue/rogue_ai_smart.kel"),
    ),
    (
        "rogue_ai_boss.kel",
        include_str!("../scripts/rogue/rogue_ai_boss.kel"),
    ),
    (
        "rogue_ai_tracker.kel",
        include_str!("../scripts/rogue/rogue_ai_tracker.kel"),
    ),
    (
        "rogue_ai_hunter.kel",
        include_str!("../scripts/rogue/rogue_ai_hunter.kel"),
    ),
    (
        "rogue_item_potion.kel",
        include_str!("../scripts/rogue/rogue_item_potion.kel"),
    ),
    (
        "rogue_item_scroll.kel",
        include_str!("../scripts/rogue/rogue_item_scroll.kel"),
    ),
    (
        "rogue_game.kel",
        include_str!("../scripts/rogue/rogue_game.kel"),
    ),
    (
        "rogue_player_ai.kel",
        include_str!("../scripts/rogue/rogue_player_ai.kel"),
    ),
    (
        "rogue_combat.kel",
        include_str!("../scripts/rogue/rogue_combat.kel"),
    ),
    (
        "rogue_book_keeping.kel",
        include_str!("../scripts/rogue/rogue_book_keeping.kel"),
    ),
    (
        "rogue_pickup.kel",
        include_str!("../scripts/rogue/rogue_pickup.kel"),
    ),
    (
        "rogue_move_resolve.kel",
        include_str!("../scripts/rogue/rogue_move_resolve.kel"),
    ),
    (
        "rogue_descend.kel",
        include_str!("../scripts/rogue/rogue_descend.kel"),
    ),
    (
        "rogue_consume.kel",
        include_str!("../scripts/rogue/rogue_consume.kel"),
    ),
    (
        "rogue_scroll_apply.kel",
        include_str!("../scripts/rogue/rogue_scroll_apply.kel"),
    ),
    (
        "rogue_bestiary.kel",
        include_str!("../scripts/rogue/rogue_bestiary.kel"),
    ),
    (
        "rogue_gear.kel",
        include_str!("../scripts/rogue/rogue_gear.kel"),
    ),
];

/// Directory containing the Keleusma script sources on disk.
/// The initial load uses the embedded constants above so the
/// example runs even without filesystem access. The hot-reload
/// path reads from this directory at run time.
pub const SCRIPT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/scripts/rogue");

/// Compile a single Keleusma source string to a [`Module`].
pub fn build_module(src: &str) -> Result<Module, Box<dyn std::error::Error>> {
    let tokens = tokenize(src).map_err(|e| format!("lex error: {:?}", e))?;
    let program = parse(&tokens).map_err(|e| format!("parse error: {:?}", e))?;
    compile(&program).map_err(|e| format!("compile error: {:?}", e).into())
}

/// Compile a script by name from the embedded table.
pub fn compile_embedded(name: &str) -> Result<Module, Box<dyn std::error::Error>> {
    let src = EMBEDDED
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, s)| *s)
        .ok_or_else(|| format!("unknown embedded script: {}", name))?;
    build_module(src)
}

/// Compile a script by name from disk under [`SCRIPT_DIR`].
pub fn compile_disk(name: &str) -> Result<Module, Box<dyn std::error::Error>> {
    let path = format!("{}/{}", SCRIPT_DIR, name);
    let src = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {}", name, e))?;
    build_module(&src)
}

/// Five-element tuple returned by item-effect scripts: hp delta,
/// max-hp delta, skill delta, status code, status argument.
pub type EffectTuple = (i64, i64, i64, i64, i64);

/// Action returned by every artificial-intelligence script. The
/// host validates the action against world state and applies it.
#[derive(Clone, Copy, Debug)]
pub enum AiAction {
    Wait,
    MoveOrMelee { tx: i32, ty: i32 },
    Ranged { tx: i32, ty: i32 },
    Descend,
    Quaff,
    Read,
}

/// Decode the script's `(action, tx, ty)` tuple into an
/// [`AiAction`]. Unknown action codes degrade to `Wait`.
pub fn decode_action(action: i64, tx: i64, ty: i64) -> AiAction {
    match action {
        1 => AiAction::MoveOrMelee {
            tx: tx as i32,
            ty: ty as i32,
        },
        2 => AiAction::Ranged {
            tx: tx as i32,
            ty: ty as i32,
        },
        3 => AiAction::Descend,
        4 => AiAction::Quaff,
        5 => AiAction::Read,
        _ => AiAction::Wait,
    }
}

/// A shared world-state handle held by the artificial-
/// intelligence subsystem so that any archetype-specific natives
/// (currently `host::rng_range` for wander) can mutate the
/// world's RNG.
pub type WorldHandle = crate::natives::WorldHandle;

/// Shared handle to the artificial-intelligence pool. Native
/// closures clone this handle and lock the inner pool to
/// dispatch per-monster artificial intelligence.
pub type AiPoolHandle = Arc<Mutex<AiPool>>;

/// Pool of artificial-intelligence virtual machines indexed by
/// archetype. Each machine owns its arena's reference. The
/// item-effect scripts share the pool because the host launches
/// them through the same dispatch pattern.
pub struct AiPool {
    pub idle: Vm<'static, 'static>,
    pub chaser: Vm<'static, 'static>,
    pub wander: Vm<'static, 'static>,
    pub sleeper: Vm<'static, 'static>,
    pub ranged: Vm<'static, 'static>,
    pub fast: Vm<'static, 'static>,
    pub smart: Vm<'static, 'static>,
    pub boss: Vm<'static, 'static>,
    pub tracker: Vm<'static, 'static>,
    pub hunter: Vm<'static, 'static>,
    pub potion: Vm<'static, 'static>,
    pub scroll: Vm<'static, 'static>,
    pub player: Vm<'static, 'static>,
    pub combat: Vm<'static, 'static>,
    pub book_keeping: Vm<'static, 'static>,
    pub pickup: Vm<'static, 'static>,
    pub move_resolve: Vm<'static, 'static>,
    pub descend: Vm<'static, 'static>,
    pub consume: Vm<'static, 'static>,
    pub scroll_apply: Vm<'static, 'static>,
    /// Has the boss's `loop main` been started yet? The host
    /// uses `vm.call` on the first turn and `vm.resume` on every
    /// subsequent turn. Reset on hot reload.
    boss_started: bool,
    /// Same flag for the Tracker `loop main` archetype.
    tracker_started: bool,
    /// Same flag for the Hunter `loop main` archetype.
    hunter_started: bool,
    /// Persistent host-owned shared-data buffers for the three `loop main`
    /// archetypes (B28 item 2). Each is lent to its virtual machine on every
    /// turn so the script's shared state survives across resumes, and is
    /// zeroed on restart by [`AiPool::reset_loop_main_data`]. A buffer is
    /// empty when its archetype declares no shared data.
    boss_shared: Vec<u8>,
    tracker_shared: Vec<u8>,
    hunter_shared: Vec<u8>,
}

impl AiPool {
    /// Construct the pool, compiling each archetype's source,
    /// instantiating the matching virtual machine, and
    /// registering the per-virtual-machine natives. Each
    /// archetype's arena is leaked to obtain a `'static`
    /// reference suitable for `Arc<Mutex<AiPool>>` sharing.
    pub fn new(
        modules: AiModules,
        world: &WorldHandle,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let idle = build_vm(modules.idle, leak_arena(), world, false)?;
        let chaser = build_vm(modules.chaser, leak_arena(), world, false)?;
        let wander = build_vm(modules.wander, leak_arena(), world, true)?;
        let sleeper = build_vm(modules.sleeper, leak_arena(), world, false)?;
        let ranged = build_vm(modules.ranged, leak_arena(), world, false)?;
        let fast = build_vm(modules.fast, leak_arena(), world, false)?;
        let smart = build_vm(modules.smart, leak_arena(), world, false)?;
        let boss = build_vm(modules.boss, leak_arena(), world, false)?;
        let tracker = build_vm(modules.tracker, leak_arena(), world, false)?;
        let hunter = build_vm(modules.hunter, leak_arena(), world, false)?;
        let potion = build_vm(modules.potion, leak_arena(), world, false)?;
        let scroll = build_vm(modules.scroll, leak_arena(), world, false)?;
        let player = build_vm(modules.player, leak_arena(), world, false)?;
        let combat = build_vm(modules.combat, leak_arena(), world, false)?;
        let book_keeping = build_vm(modules.book_keeping, leak_arena(), world, false)?;
        let pickup = build_vm(modules.pickup, leak_arena(), world, false)?;
        let move_resolve = build_vm(modules.move_resolve, leak_arena(), world, false)?;
        let descend = build_vm(modules.descend, leak_arena(), world, false)?;
        let mut consume = build_vm(modules.consume, leak_arena(), world, false)?;
        let mut scroll_apply = build_vm(modules.scroll_apply, leak_arena(), world, false)?;
        crate::natives::register_consume_natives(&mut consume, world);
        crate::natives::register_scroll_apply_natives(&mut scroll_apply, world);
        let boss_shared = vec![0u8; boss.shared_data_bytes()];
        let tracker_shared = vec![0u8; tracker.shared_data_bytes()];
        let hunter_shared = vec![0u8; hunter.shared_data_bytes()];
        Ok(Self {
            idle,
            chaser,
            wander,
            sleeper,
            ranged,
            fast,
            smart,
            boss,
            tracker,
            hunter,
            potion,
            scroll,
            player,
            combat,
            book_keeping,
            pickup,
            move_resolve,
            descend,
            consume,
            scroll_apply,
            boss_started: false,
            tracker_started: false,
            hunter_started: false,
            boss_shared,
            tracker_shared,
            hunter_shared,
        })
    }

    /// Replace every virtual machine in the pool with one built
    /// from the supplied modules. Each replacement leaks a fresh
    /// arena. The drop of the previous virtual machine releases
    /// the prior arena's bottom region; the leaked arena memory
    /// itself stays leaked but the size is constant so the
    /// long-run behaviour is bounded.
    pub fn reload(
        &mut self,
        modules: AiModules,
        world: &WorldHandle,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.idle = build_vm(modules.idle, leak_arena(), world, false)?;
        self.chaser = build_vm(modules.chaser, leak_arena(), world, false)?;
        self.wander = build_vm(modules.wander, leak_arena(), world, true)?;
        self.sleeper = build_vm(modules.sleeper, leak_arena(), world, false)?;
        self.ranged = build_vm(modules.ranged, leak_arena(), world, false)?;
        self.fast = build_vm(modules.fast, leak_arena(), world, false)?;
        self.smart = build_vm(modules.smart, leak_arena(), world, false)?;
        self.boss = build_vm(modules.boss, leak_arena(), world, false)?;
        self.tracker = build_vm(modules.tracker, leak_arena(), world, false)?;
        self.hunter = build_vm(modules.hunter, leak_arena(), world, false)?;
        self.potion = build_vm(modules.potion, leak_arena(), world, false)?;
        self.scroll = build_vm(modules.scroll, leak_arena(), world, false)?;
        self.player = build_vm(modules.player, leak_arena(), world, false)?;
        self.combat = build_vm(modules.combat, leak_arena(), world, false)?;
        self.book_keeping = build_vm(modules.book_keeping, leak_arena(), world, false)?;
        self.pickup = build_vm(modules.pickup, leak_arena(), world, false)?;
        self.move_resolve = build_vm(modules.move_resolve, leak_arena(), world, false)?;
        self.descend = build_vm(modules.descend, leak_arena(), world, false)?;
        let mut consume = build_vm(modules.consume, leak_arena(), world, false)?;
        let mut scroll_apply = build_vm(modules.scroll_apply, leak_arena(), world, false)?;
        crate::natives::register_consume_natives(&mut consume, world);
        crate::natives::register_scroll_apply_natives(&mut scroll_apply, world);
        self.consume = consume;
        self.scroll_apply = scroll_apply;
        self.boss_started = false;
        self.tracker_started = false;
        self.hunter_started = false;
        // Re-size the shared buffers to the freshly built modules and clear
        // them; a hot reload starts each loop-main archetype's shared state
        // from zero.
        self.boss_shared = vec![0u8; self.boss.shared_data_bytes()];
        self.tracker_shared = vec![0u8; self.tracker.shared_data_bytes()];
        self.hunter_shared = vec![0u8; self.hunter.shared_data_bytes()];
        Ok(())
    }

    /// Invoke the player artificial-intelligence virtual machine
    /// with the player's current position and latest keypress.
    /// Returns the action tuple in the same shape every other
    /// artificial-intelligence script emits.
    pub fn dispatch_player(
        &mut self,
        mx: i32,
        my: i32,
        cmd: i64,
    ) -> Result<AiAction, Box<dyn std::error::Error>> {
        let t = call_pure_ints(&mut self.player, "player", &[mx as i64, my as i64, cmd], 3)?;
        Ok(decode_action(t[0], t[1], t[2]))
    }

    /// Zero the data segments of every `loop main` archetype's
    /// virtual machine. Used by restart so memory the boss,
    /// tracker, or hunter scripts kept across the previous run
    /// does not bleed into the new run. Each virtual machine's
    /// position is unchanged; the next resume re-enters the
    /// loop body and reads the zeroed state.
    pub fn reset_loop_main_data(&mut self) {
        for buf in [
            &mut self.boss_shared,
            &mut self.tracker_shared,
            &mut self.hunter_shared,
        ] {
            buf.iter_mut().for_each(|b| *b = 0);
        }
    }

    /// Invoke the book-keeping virtual machine. Returns the
    /// post-tick `(hp, hunger)` pair given the new turn number
    /// and the pre-tick state.
    pub fn dispatch_book_keeping(
        &mut self,
        turn: i64,
        hp: i64,
        max_hp: i64,
        hunger: i64,
    ) -> Result<(i64, i64, i64), Box<dyn std::error::Error>> {
        let t = call_pure_ints(
            &mut self.book_keeping,
            "book",
            &[turn, hp, max_hp, hunger],
            3,
        )?;
        Ok((t[0], t[1], t[2]))
    }

    /// Invoke the pickup-decision virtual machine. Returns the
    /// pickup action code: 0 = leave, 1 = consume / equip / slot,
    /// 2 = scrap (non-upgrade gear).
    pub fn dispatch_pickup(
        &mut self,
        item_kind: i64,
        new_value: i64,
        current_value: i64,
        slot_full: i64,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        call_pure_int(
            &mut self.pickup,
            "pickup",
            &[item_kind, new_value, current_value, slot_full],
        )
    }

    /// Invoke the move-resolution virtual machine. Returns
    /// 0 = blocked, 1 = step, 2 = attack.
    pub fn dispatch_move_resolve(
        &mut self,
        tile: i64,
        monster_at_target: i64,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        call_pure_int(&mut self.move_resolve, "move", &[tile, monster_at_target])
    }

    /// Invoke the combat virtual machine. Returns `(hit_kind,
    /// damage)` where hit_kind is 0 miss, 1 hit, 2 critical.
    pub fn dispatch_combat(
        &mut self,
        attacker_skill: i64,
        attacker_damage: i64,
        defender_evasion: i64,
        defender_armor: i64,
        roll: i64,
    ) -> Result<(i64, i64), Box<dyn std::error::Error>> {
        let t = call_pure_ints(
            &mut self.combat,
            "combat",
            &[
                attacker_skill,
                attacker_damage,
                defender_evasion,
                defender_armor,
                roll,
            ],
            2,
        )?;
        Ok((t[0], t[1]))
    }

    /// Invoke the potion-effect virtual machine. Returns the
    /// five-element tuple `(hp_delta, max_hp_delta, skill_delta,
    /// status_code, status_arg)`.
    pub fn dispatch_potion(
        &mut self,
        effect: i64,
        hp: i64,
        max_hp: i64,
    ) -> Result<EffectTuple, Box<dyn std::error::Error>> {
        call_pure_5(&mut self.potion, "potion", &[effect, hp, max_hp])
    }

    /// Invoke the scroll-effect virtual machine.
    pub fn dispatch_scroll(
        &mut self,
        effect: i64,
    ) -> Result<EffectTuple, Box<dyn std::error::Error>> {
        call_pure_5(&mut self.scroll, "scroll", &[effect])
    }

    /// Invoke the descend script. Returns the post-descent
    /// `(level, max_hp, hp, skill, floor)`.
    pub fn dispatch_descend(
        &mut self,
        level: i64,
        max_hp: i64,
        hp: i64,
        skill: i64,
        floor: i64,
    ) -> Result<DescendOutputs, Box<dyn std::error::Error>> {
        let t = call_pure_ints(
            &mut self.descend,
            "descend",
            &[level, max_hp, hp, skill, floor],
            5,
        )?;
        Ok((t[0], t[1], t[2], t[3], t[4]))
    }

    /// Invoke the consume script. Returns nothing meaningful;
    /// the script's effect is the series of fine-grained native
    /// calls that mutate the world and push messages.
    pub fn dispatch_consume(
        &mut self,
        kind: i64,
        subtype: i64,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        call_pure_int(&mut self.consume, "consume", &[kind, subtype])
    }

    /// Invoke the scroll-apply script. Same shape as
    /// `dispatch_consume`. Side effects flow through the
    /// fine-grained scroll-apply natives.
    pub fn dispatch_scroll_apply(
        &mut self,
        code: i64,
        arg: i64,
    ) -> Result<i64, Box<dyn std::error::Error>> {
        call_pure_int(&mut self.scroll_apply, "scroll_apply", &[code, arg])
    }

    pub fn vm_for(&mut self, ai: AiKind) -> &mut Vm<'static, 'static> {
        match ai {
            AiKind::Idle => &mut self.idle,
            AiKind::Chaser => &mut self.chaser,
            AiKind::Wander => &mut self.wander,
            AiKind::Sleeper => &mut self.sleeper,
            AiKind::Ranged => &mut self.ranged,
            AiKind::Fast => &mut self.fast,
            AiKind::Smart => &mut self.smart,
            AiKind::Boss => &mut self.boss,
            AiKind::Tracker => &mut self.tracker,
            AiKind::Hunter => &mut self.hunter,
        }
    }

    /// Invoke the archetype's virtual machine with the supplied
    /// monster and world state. The script returns a `(action,
    /// tx, ty)` tuple decoded into [`AiAction`].
    pub fn dispatch(
        &mut self,
        ai: AiKind,
        mx: i32,
        my: i32,
        px: i32,
        py: i32,
        sees: bool,
    ) -> Result<AiAction, Box<dyn std::error::Error>> {
        if matches!(ai, AiKind::Boss) {
            return self.dispatch_loop_main(LoopMainKind::Boss, mx, my, px, py, sees);
        }
        if matches!(ai, AiKind::Tracker) {
            return self.dispatch_loop_main(LoopMainKind::Tracker, mx, my, px, py, sees);
        }
        if matches!(ai, AiKind::Hunter) {
            return self.dispatch_loop_main(LoopMainKind::Hunter, mx, my, px, py, sees);
        }
        let vm = self.vm_for(ai);
        // The VM's arena outlives this borrow (`arena()` returns the arena's
        // own `'static` lifetime); a flat tuple result resolves against it
        // (B28 item 2 step 6B).
        let arena = vm.arena();
        let args = [
            Value::Int(mx as i64),
            Value::Int(my as i64),
            Value::Int(px as i64),
            Value::Int(py as i64),
            Value::Int(if sees { 1 } else { 0 }),
        ];
        let result = vm.call(&args).map_err(|e| format!("ai vm: {:?}", e))?;
        match result {
            VmState::Finished(val @ Value::Tuple(_)) => {
                let t = tuple_ints(&val, 3, arena)?;
                Ok(decode_action(t[0], t[1], t[2]))
            }
            other => Err(format!("ai vm returned unexpected shape: {:?}", other).into()),
        }
    }

    /// Dispatch a `loop main` archetype. The first turn uses
    /// `vm.call`; every subsequent turn uses `vm.resume`. Because
    /// Keleusma emits `Reset` when the loop body wraps from end
    /// to top, the host loops past `Reset` until the next
    /// `Yielded` to retrieve one action per logical turn.
    fn dispatch_loop_main(
        &mut self,
        kind: LoopMainKind,
        mx: i32,
        my: i32,
        px: i32,
        py: i32,
        sees: bool,
    ) -> Result<AiAction, Box<dyn std::error::Error>> {
        // Build the input through the shared constructor so the tuple has
        // the same representation a script-built tuple of the same type
        // would, which the compiled `GetTupleField` offsets rely on.
        let input = Value::tuple(alloc::vec![
            Value::Int(mx as i64),
            Value::Int(my as i64),
            Value::Int(px as i64),
            Value::Int(py as i64),
            Value::Int(if sees { 1 } else { 0 }),
        ]);
        let (vm, started_flag, shared): (&mut Vm<'static, 'static>, &mut bool, &mut Vec<u8>) =
            match kind {
                LoopMainKind::Boss => (
                    &mut self.boss,
                    &mut self.boss_started,
                    &mut self.boss_shared,
                ),
                LoopMainKind::Tracker => (
                    &mut self.tracker,
                    &mut self.tracker_started,
                    &mut self.tracker_shared,
                ),
                LoopMainKind::Hunter => (
                    &mut self.hunter,
                    &mut self.hunter_started,
                    &mut self.hunter_shared,
                ),
            };
        // The VM's arena outlives this borrow (`arena()` returns the arena's
        // own `'static` lifetime); a flat tuple yield resolves against it
        // (B28 item 2 step 6B).
        let arena = vm.arena();
        // Lend the archetype's persistent shared buffer for this turn; the
        // script reads and writes it in place, so its shared state carries
        // across resumes (B28 item 2).
        let mut state = if *started_flag {
            vm.resume_with_shared(shared, input.clone())
        } else {
            *started_flag = true;
            vm.call_with_shared(shared, std::slice::from_ref(&input))
        }
        .map_err(|e| format!("loop main vm: {:?}", e))?;
        for _ in 0..16 {
            match state {
                VmState::Yielded(val @ Value::Tuple(_)) => {
                    let t = tuple_ints(&val, 3, arena)?;
                    return Ok(decode_action(t[0], t[1], t[2]));
                }
                VmState::Reset => {
                    state = vm
                        .resume_with_shared(shared, input.clone())
                        .map_err(|e| format!("loop main vm: {:?}", e))?;
                }
                VmState::Finished(_) => return Err("loop main vm finished unexpectedly".into()),
                other => {
                    return Err(
                        format!("loop main vm returned unexpected shape: {:?}", other).into(),
                    );
                }
            }
        }
        Err("loop main vm exhausted Reset budget without yielding".into())
    }
}

/// Discriminator for [`AiPool::dispatch_loop_main`]. Each
/// variant selects the matching virtual machine and started
/// flag.
enum LoopMainKind {
    Boss,
    Tracker,
    Hunter,
}

fn expect_int(v: &Value) -> Result<i64, Box<dyn std::error::Error>> {
    match v {
        Value::Int(n) => Ok(*n),
        other => Err(format!("expected Word, got {:?}", other).into()),
    }
}

/// Extract the integer fields of a tuple `Value` returned by an
/// artificial-intelligence script. A transitively-scalar tuple is stored
/// as a flat little-endian byte body (B28); each `Word` field is read back
/// at its packed offset using the bundled runtime's eight-byte word, the
/// same width the marshalling layer applies. The boxed body, used when a
/// tuple is not all flat scalars, is also accepted.
fn tuple_ints(
    v: &Value,
    expected: usize,
    arena: &Arena,
) -> Result<Vec<i64>, Box<dyn std::error::Error>> {
    use keleusma::bytecode::TupleBody;
    use keleusma::value_layout::ScalarKind;
    // Bundled runtime widths: `Value = GenericValue<i64, f64>`.
    const WORD_BYTES: usize = 8;
    const FLOAT_BYTES: usize = 8;
    let ints: Vec<i64> = match v {
        Value::Tuple(TupleBody::Boxed(items)) => {
            items.iter().map(expect_int).collect::<Result<_, _>>()?
        }
        Value::Tuple(TupleBody::Flat(fc)) => {
            // A flat tuple body is an arena region handle (B28 item 2 step 6B);
            // resolve it against the same arena the VM returned it in, valid
            // until the next reset (read-before-resume).
            let bytes = fc
                .resolve(arena)
                .map_err(|_| "flat tuple body is stale (arena reset)")?;
            if bytes.len() != expected * WORD_BYTES {
                return Err(format!(
                    "expected a flat tuple of {} words, got {} bytes",
                    expected,
                    bytes.len()
                )
                .into());
            }
            (0..expected)
                .map(|i| {
                    let v = Value::read_scalar_le(
                        bytes,
                        i * WORD_BYTES,
                        ScalarKind::Int,
                        WORD_BYTES,
                        FLOAT_BYTES,
                    )
                    .map_err(|e| -> Box<dyn std::error::Error> {
                        format!("flat tuple element decode failed: {:?}", e).into()
                    })?;
                    expect_int(&v)
                })
                .collect::<Result<_, _>>()?
        }
        other => return Err(format!("expected tuple, got {:?}", other).into()),
    };
    if ints.len() != expected {
        return Err(format!("expected tuple of arity {}, got {}", expected, ints.len()).into());
    }
    Ok(ints)
}

/// Unpack a `Finished` virtual-machine state whose value is a
/// tuple of `n` integers. Used by the pure-function dispatch
/// helpers to share the result-decoding boilerplate.
fn unpack_finished_ints(
    result: VmState,
    name: &str,
    n: usize,
    arena: &Arena,
) -> Result<Vec<i64>, Box<dyn std::error::Error>> {
    match result {
        VmState::Finished(val @ Value::Tuple(_)) => tuple_ints(&val, n, arena)
            .map_err(|e| format!("{} vm returned unexpected shape: {}", name, e).into()),
        other => Err(format!("{} vm returned unexpected shape: {:?}", name, other).into()),
    }
}

/// Unpack a `Finished` virtual-machine state whose value is a
/// single integer.
fn unpack_finished_int(result: VmState, name: &str) -> Result<i64, Box<dyn std::error::Error>> {
    match result {
        VmState::Finished(Value::Int(n)) => Ok(n),
        other => Err(format!("{} vm returned unexpected shape: {:?}", name, other).into()),
    }
}

fn unpack_5_tuple(
    result: VmState,
    arena: &Arena,
) -> Result<EffectTuple, Box<dyn std::error::Error>> {
    match result {
        VmState::Finished(val @ Value::Tuple(_)) => {
            let t = tuple_ints(&val, 5, arena)?;
            Ok((t[0], t[1], t[2], t[3], t[4]))
        }
        other => Err(format!("expected 5-tuple, got {:?}", other).into()),
    }
}

/// Call a pure-function virtual machine with integer arguments
/// and return its raw `VmState`. Used by the typed wrappers
/// below to share the value-wrap, call, and error-format step.
fn call_pure(
    vm: &mut Vm<'static, 'static>,
    name: &str,
    args: &[i64],
) -> Result<VmState, Box<dyn std::error::Error>> {
    let values: Vec<Value> = args.iter().map(|n| Value::Int(*n)).collect();
    vm.call(&values)
        .map_err(|e| format!("{} vm: {:?}", name, e).into())
}

fn call_pure_int(
    vm: &mut Vm<'static, 'static>,
    name: &str,
    args: &[i64],
) -> Result<i64, Box<dyn std::error::Error>> {
    unpack_finished_int(call_pure(vm, name, args)?, name)
}

fn call_pure_ints(
    vm: &mut Vm<'static, 'static>,
    name: &str,
    args: &[i64],
    n: usize,
) -> Result<Vec<i64>, Box<dyn std::error::Error>> {
    let state = call_pure(vm, name, args)?;
    unpack_finished_ints(state, name, n, vm.arena())
}

fn call_pure_5(
    vm: &mut Vm<'static, 'static>,
    name: &str,
    args: &[i64],
) -> Result<EffectTuple, Box<dyn std::error::Error>> {
    let state = call_pure(vm, name, args)?;
    unpack_5_tuple(state, vm.arena())
}

/// Leak a fresh arena so the host can reference it for the
/// remaining lifetime of the program. The host needs `'static`
/// references because the pool is wrapped in `Arc<Mutex<>>` and
/// shared with native closures.
fn leak_arena() -> &'static Arena {
    Box::leak(Box::new(Arena::with_capacity(DEFAULT_ARENA_CAPACITY)))
}

/// Per-archetype compiled modules. Construction is performed at
/// startup before the artificial-intelligence pool is built.
pub struct AiModules {
    pub idle: Module,
    pub chaser: Module,
    pub wander: Module,
    pub sleeper: Module,
    pub ranged: Module,
    pub fast: Module,
    pub smart: Module,
    pub boss: Module,
    pub tracker: Module,
    pub hunter: Module,
    pub potion: Module,
    pub scroll: Module,
    pub player: Module,
    pub combat: Module,
    pub book_keeping: Module,
    pub pickup: Module,
    pub move_resolve: Module,
    pub descend: Module,
    pub consume: Module,
    pub scroll_apply: Module,
}

impl AiModules {
    /// Build the full set of artificial-intelligence and item
    /// modules using a caller-supplied compilation step. The
    /// closure receives a script filename and returns the
    /// compiled `Module` or an error. The same constructor
    /// drives both startup (compiling embedded `include_str!`
    /// sources) and hot reload (reading sources from disk).
    pub fn build<F>(mut compile: F) -> Result<Self, Box<dyn std::error::Error>>
    where
        F: FnMut(&str) -> Result<Module, Box<dyn std::error::Error>>,
    {
        Ok(Self {
            idle: compile("rogue_ai_idle.kel")?,
            chaser: compile("rogue_ai_chaser.kel")?,
            wander: compile("rogue_ai_wander.kel")?,
            sleeper: compile("rogue_ai_sleeper.kel")?,
            ranged: compile("rogue_ai_ranged.kel")?,
            fast: compile("rogue_ai_fast.kel")?,
            smart: compile("rogue_ai_smart.kel")?,
            boss: compile("rogue_ai_boss.kel")?,
            tracker: compile("rogue_ai_tracker.kel")?,
            hunter: compile("rogue_ai_hunter.kel")?,
            potion: compile("rogue_item_potion.kel")?,
            scroll: compile("rogue_item_scroll.kel")?,
            player: compile("rogue_player_ai.kel")?,
            combat: compile("rogue_combat.kel")?,
            book_keeping: compile("rogue_book_keeping.kel")?,
            pickup: compile("rogue_pickup.kel")?,
            move_resolve: compile("rogue_move_resolve.kel")?,
            descend: compile("rogue_descend.kel")?,
            consume: compile("rogue_consume.kel")?,
            scroll_apply: compile("rogue_scroll_apply.kel")?,
        })
    }
}

fn build_vm(
    module: Module,
    arena: &'static Arena,
    world: &WorldHandle,
    needs_rng: bool,
) -> Result<Vm<'static, 'static>, Box<dyn std::error::Error>> {
    let mut vm = Vm::new(module, arena).map_err(|e| format!("vm new: {:?}", e))?;
    // Shared data, where an archetype declares it, is held in a host-owned
    // buffer allocated zeroed by the pool (B28 item 2); no slot zeroing here.
    if needs_rng {
        register_rng(&mut vm, world);
    }
    Ok(vm)
}

fn register_rng(vm: &mut Vm, world: &WorldHandle) {
    let w: Arc<Mutex<_>> = world.clone();
    vm.register_native_closure(
        "host::rng_range",
        Box::new(move |args: &[Value]| -> Result<Value, VmError> {
            if args.len() != 2 {
                return Err(VmError::NativeError(format!(
                    "host::rng_range: expected 2 args, got {}",
                    args.len()
                )));
            }
            let lo = match args[0] {
                Value::Int(n) => n,
                _ => {
                    return Err(VmError::NativeError(String::from(
                        "host::rng_range: lo must be Word",
                    )));
                }
            };
            let hi = match args[1] {
                Value::Int(n) => n,
                _ => {
                    return Err(VmError::NativeError(String::from(
                        "host::rng_range: hi must be Word",
                    )));
                }
            };
            if hi <= lo {
                return Err(VmError::NativeError(format!(
                    "host::rng_range: hi {} must be greater than lo {}",
                    hi, lo
                )));
            }
            let mut world = w.lock().unwrap();
            let span = (hi - lo) as u32;
            let r = world.rng_next() % span;
            Ok(Value::Int(lo + r as i64))
        }),
    );
}
