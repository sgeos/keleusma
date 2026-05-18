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
use keleusma::vm::{DEFAULT_ARENA_CAPACITY, Vm, VmError, VmState};
use keleusma::{Arena, Module};

use crate::bestiary::AiKind;

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
    pub potion: Vm<'static, 'static>,
    pub scroll: Vm<'static, 'static>,
    pub player: Vm<'static, 'static>,
    pub combat: Vm<'static, 'static>,
    /// Has the boss's `loop main` been started yet? The host
    /// uses `vm.call` on the first turn and `vm.resume` on every
    /// subsequent turn. Reset on hot reload.
    boss_started: bool,
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
        let potion = build_vm(modules.potion, leak_arena(), world, false)?;
        let scroll = build_vm(modules.scroll, leak_arena(), world, false)?;
        let player = build_vm(modules.player, leak_arena(), world, false)?;
        let combat = build_vm(modules.combat, leak_arena(), world, false)?;
        Ok(Self {
            idle,
            chaser,
            wander,
            sleeper,
            ranged,
            fast,
            smart,
            boss,
            potion,
            scroll,
            player,
            combat,
            boss_started: false,
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
        self.potion = build_vm(modules.potion, leak_arena(), world, false)?;
        self.scroll = build_vm(modules.scroll, leak_arena(), world, false)?;
        self.player = build_vm(modules.player, leak_arena(), world, false)?;
        self.combat = build_vm(modules.combat, leak_arena(), world, false)?;
        self.boss_started = false;
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
        let args = [
            Value::Int(mx as i64),
            Value::Int(my as i64),
            Value::Int(cmd),
        ];
        let result = self
            .player
            .call(&args)
            .map_err(|e| format!("player vm: {:?}", e))?;
        match result {
            VmState::Finished(Value::Tuple(t)) if t.len() == 3 => {
                let a = expect_int(&t[0])?;
                let x = expect_int(&t[1])?;
                let y = expect_int(&t[2])?;
                Ok(decode_action(a, x, y))
            }
            other => Err(format!("player vm returned unexpected shape: {:?}", other).into()),
        }
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
        let args = [
            Value::Int(attacker_skill),
            Value::Int(attacker_damage),
            Value::Int(defender_evasion),
            Value::Int(defender_armor),
            Value::Int(roll),
        ];
        let result = self
            .combat
            .call(&args)
            .map_err(|e| format!("combat vm: {:?}", e))?;
        match result {
            VmState::Finished(Value::Tuple(t)) if t.len() == 2 => {
                Ok((expect_int(&t[0])?, expect_int(&t[1])?))
            }
            other => Err(format!("combat vm returned unexpected shape: {:?}", other).into()),
        }
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
        let args = [Value::Int(effect), Value::Int(hp), Value::Int(max_hp)];
        let result = self
            .potion
            .call(&args)
            .map_err(|e| format!("potion vm: {:?}", e))?;
        unpack_5_tuple(result)
    }

    /// Invoke the scroll-effect virtual machine.
    pub fn dispatch_scroll(
        &mut self,
        effect: i64,
    ) -> Result<EffectTuple, Box<dyn std::error::Error>> {
        let args = [Value::Int(effect)];
        let result = self
            .scroll
            .call(&args)
            .map_err(|e| format!("scroll vm: {:?}", e))?;
        unpack_5_tuple(result)
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
            return self.dispatch_boss(mx, my, px, py, sees);
        }
        let vm = self.vm_for(ai);
        let args = [
            Value::Int(mx as i64),
            Value::Int(my as i64),
            Value::Int(px as i64),
            Value::Int(py as i64),
            Value::Int(if sees { 1 } else { 0 }),
        ];
        let result = vm.call(&args).map_err(|e| format!("ai vm: {:?}", e))?;
        match result {
            VmState::Finished(Value::Tuple(t)) if t.len() == 3 => {
                let a = expect_int(&t[0])?;
                let x = expect_int(&t[1])?;
                let y = expect_int(&t[2])?;
                Ok(decode_action(a, x, y))
            }
            other => Err(format!("ai vm returned unexpected shape: {:?}", other).into()),
        }
    }

    /// Dispatch the boss archetype. See the historical
    /// implementation in `dispatch_boss` for details. The script
    /// is `loop main`; the host calls on the first turn and
    /// resumes thereafter, walking past `Reset` to the next
    /// `Yielded`.
    fn dispatch_boss(
        &mut self,
        mx: i32,
        my: i32,
        px: i32,
        py: i32,
        sees: bool,
    ) -> Result<AiAction, Box<dyn std::error::Error>> {
        let input = Value::Tuple(alloc::vec![
            Value::Int(mx as i64),
            Value::Int(my as i64),
            Value::Int(px as i64),
            Value::Int(py as i64),
            Value::Int(if sees { 1 } else { 0 }),
        ]);
        let mut state = if self.boss_started {
            self.boss.resume(input.clone())
        } else {
            self.boss_started = true;
            self.boss.call(std::slice::from_ref(&input))
        }
        .map_err(|e| format!("boss vm: {:?}", e))?;
        for _ in 0..16 {
            match state {
                VmState::Yielded(Value::Tuple(t)) if t.len() == 3 => {
                    let a = expect_int(&t[0])?;
                    let x = expect_int(&t[1])?;
                    let y = expect_int(&t[2])?;
                    return Ok(decode_action(a, x, y));
                }
                VmState::Reset => {
                    state = self
                        .boss
                        .resume(input.clone())
                        .map_err(|e| format!("boss vm: {:?}", e))?;
                }
                VmState::Finished(_) => return Err("boss vm finished unexpectedly".into()),
                other => {
                    return Err(format!("boss vm returned unexpected shape: {:?}", other).into());
                }
            }
        }
        Err("boss vm exhausted Reset budget without yielding".into())
    }
}

fn expect_int(v: &Value) -> Result<i64, Box<dyn std::error::Error>> {
    match v {
        Value::Int(n) => Ok(*n),
        other => Err(format!("expected Word, got {:?}", other).into()),
    }
}

fn unpack_5_tuple(result: VmState) -> Result<EffectTuple, Box<dyn std::error::Error>> {
    match result {
        VmState::Finished(Value::Tuple(t)) if t.len() == 5 => Ok((
            expect_int(&t[0])?,
            expect_int(&t[1])?,
            expect_int(&t[2])?,
            expect_int(&t[3])?,
            expect_int(&t[4])?,
        )),
        other => Err(format!("expected 5-tuple, got {:?}", other).into()),
    }
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
    pub potion: Module,
    pub scroll: Module,
    pub player: Module,
    pub combat: Module,
}

fn build_vm(
    module: Module,
    arena: &'static Arena,
    world: &WorldHandle,
    needs_rng: bool,
) -> Result<Vm<'static, 'static>, Box<dyn std::error::Error>> {
    let mut vm = Vm::new(module, arena).map_err(|e| format!("vm new: {:?}", e))?;
    for slot in 0..vm.data_len() {
        let _ = vm.set_data(slot, Value::Int(0));
    }
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
