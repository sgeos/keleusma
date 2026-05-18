# Keleusma Cookbook

> **Navigation**: [Guide](./README.md) | [Documentation Root](../README.md)

Recipes are working patterns for embedding Keleusma in larger systems. Each recipe states the problem it solves, the constraint it respects, and a minimal working example. The shipped rogue and piano-roll examples are referenced where they instantiate the pattern at production scale.

## Index

| Recipe | Use it when |
|--------|-------------|
| [The data-loader pattern](#the-data-loader-pattern) | The host needs read-only configuration data that benefits from script-side editing. |

---

## The data-loader pattern

### Problem

The host needs a table of configuration data. The data is structurally homogeneous (a fixed-shape record per entry) but designer-tunable (game balance, look-up tables, content). Storing the table in Rust source means designers must rebuild the host to retune. Storing it in a script file lets designers edit a `.kel` file and reload at runtime.

Keleusma does not currently support module-scope `const` declarations for arrays of records, inline string tables, or runtime allocation of growable structures. The pattern below works inside those constraints.

### Solution

Encode the table as a Keleusma script with three pieces.

1. **A data segment** declared on the script side, holding one field per output column of the record. The data segment is the host-script I/O struct.
2. **A multi-headed dispatcher** with one head per entry. Each head writes the per-entry constants into the data segment.
3. **A loader function** that resolves the index (including the negative-index convention) and chains into the dispatcher.

The host runs the script once per entry at startup, reads the data segment after each call, and caches the result in a regular Rust container (`Vec<T>`, `HashMap<K, T>`, or similar). After the cache is warm, runtime reads go through the Rust cache; the script is touched again only when the host wants to reload.

The pattern admits runtime hot reload because the table is in script form. A host that re-compiles the script, re-runs the loader, and atomically replaces the cache can swap data without restarting. The rogue example caches once at startup and does not currently reload the bestiary, but the pattern itself does not preclude reload.

### Three component techniques

The pattern composes three techniques that are individually known but compose well.

**Multi-headed dispatch encoding a constant table.** Keleusma admits multi-headed function definitions with integer-pattern parameters. One head per entry, each body assigning the entry's fields, is functionally equivalent to a constant array. The encoding is verifier-friendly because every body is straight-line code. Prolog facts and Erlang or Elixir pattern matching are close analogues.

**Data segment as host-script I/O struct.** The data segment is normally the place where a `loop main` script preserves state across resumes. Repurposing it for one-shot pure functions as an output struct works because `get_data` and `set_data` are already part of the host boundary. The script reads the input through its function argument and writes outputs through `state.field = ...` assignments.

**Negative-index size discovery.** The loader resolves negative indices to `count + n` (Python sequence convention). Calling `fn main(-1)` writes the last entry's fields, including an `id` slot equal to `count - 1`. The host reads the `id` slot to learn the table size with one call, sizes its cache from that, and asserts the value against any parallel host-side constant. This avoids hard-coding the count in the Rust source.

### Minimal example

A table of three colours, each with red, green, blue channels.

```keleusma
// rogue_colours.kel
data state {
    id: Word,
    r: Word, g: Word, b: Word,
}

fn main(n: Word) -> Word {
    let count = 3;
    let i = if n < 0 { count + n } else { n };
    fill(i);
    0
}

fn fill(0) -> Word { state.id = 0; state.r = 255; state.g =   0; state.b =   0; 0 }  // red
fn fill(1) -> Word { state.id = 1; state.r =   0; state.g = 255; state.b =   0; 0 }  // green
fn fill(2) -> Word { state.id = 2; state.r =   0; state.g =   0; state.b = 255; 0 }  // blue
fn fill(_n: Word) -> Word { 0 }
```

Host side, with the cache discovered from the script.

```rust
use std::sync::OnceLock;

pub struct Colour { pub r: u8, pub g: u8, pub b: u8 }

static COLOURS: OnceLock<Vec<Colour>> = OnceLock::new();

pub fn colours() -> &'static [Colour] {
    COLOURS.get().expect("colours not loaded")
}

fn load_colours(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    // Discover the count by calling with -1.
    vm.call(&[Value::Int(-1)])?;
    let count = read_int(vm, 0)? as usize + 1;
    let mut table = Vec::with_capacity(count);
    for i in 0..count {
        vm.call(&[Value::Int(i as i64)])?;
        table.push(Colour {
            r: read_int(vm, 1)? as u8,
            g: read_int(vm, 2)? as u8,
            b: read_int(vm, 3)? as u8,
        });
    }
    let _ = COLOURS.set(table);
    Ok(())
}

fn read_int(vm: &Vm, slot: usize) -> Result<i64, Box<dyn std::error::Error>> {
    match vm.get_data(slot)? {
        Value::Int(n) => Ok(*n),
        other => Err(format!("expected Int at slot {}, got {:?}", slot, other).into()),
    }
}
```

### Variations

**Multiple tables in one script.** If two tables share the same data-segment shape, dispatch on a leading `table` argument. The rogue example's `rogue_gear.kel` does this: `fn main(table, tier)` dispatches `weapon(tier)` or `armor(tier)` based on `table`. Each table is independently discoverable via `-1`.

```keleusma
fn main(table: Word, tier: Word) -> Word {
    let count = 20;
    let i = if tier < 0 { count + tier } else { tier };
    if table == 0 { weapon(i); }
    else { if table == 1 { armor(i); } };
    0
}

fn weapon(0) -> Word { ... }
fn armor(0) -> Word { ... }
```

**Chained dispatchers.** When some output fields are derived from others, chain two dispatchers in the loader. The rogue bestiary script does this: `fn main(n)` calls `fill(i)` to set base stats including a `shape` field, then chains `corpse_fill(state.shape)` to derive three additional fields from the shape. The host receives a fully populated entry from a single call.

```keleusma
fn main(n: Word) -> Word {
    let count = 100;
    let i = if n < 0 { count + n } else { n };
    fill(i);
    corpse_fill(state.shape);
    0
}
```

**Names outside the script.** Strings in Keleusma data segments are not currently supported. When entries have a name field, keep the names in a parallel host-side `const NAMES: [&str; N]` array and assert during loading that `count == NAMES.len()`. The rogue bestiary and gear scripts both do this.

### When to use

The pattern fits when all of the following hold.

- The table has more than about ten entries. Below that, the script overhead exceeds the savings.
- Each entry is a small struct of integers or enum ordinals. Strings, floats with quirky precision, or variable-size payloads need workarounds.
- The data benefits from being designer-editable without a host rebuild. If only the Rust author ever touches the table, leave it in Rust.
- Runtime hot reload is desirable, even if the initial implementation caches once. The pattern keeps the path open.

### When not to use

- The data is already dense in Rust (one line per entry with no per-entry struct boilerplate). The migration adds script-loading overhead without compressing the storage.
- The data has lifecycle hooks (constructors, drop). Keleusma cannot carry those. Keep them in Rust.
- The data is keyed on a type that the script cannot represent. Strings, floats with specific precision requirements, or compound keys all push the pattern out of fit.

### Production examples in this repository

- `examples/scripts/rogue/rogue_bestiary.kel` is the largest worked instance. One hundred monster entries plus a twelve-entry corpse-shape sub-table. The host caches in `examples/rogue/bestiary.rs::BESTIARY: OnceLock<Vec<MonsterKind>>`. See [ROGUE.md, *Reading the bestiary script*](./ROGUE.md#reading-the-bestiary-script).
- `examples/scripts/rogue/rogue_gear.kel` is the two-tables-in-one-script variant. Twenty weapons and twenty armors with one numeric value per entry. The host caches in `examples/rogue/items.rs::WEAPONS` and `ARMORS` (both `OnceLock<Vec<_>>`).

Both scripts include their dispatcher tables inline (one line per entry); editing a damage value or a monster's hit points is a one-line change in the script. Modders can retune without rebuilding the host. The example does not yet wire the bestiary or gear scripts into the F5 hot reload path, but the pattern admits it; an exercise for a future iteration is to lift those scripts into the reload chain alongside the artificial-intelligence and item-effect scripts.
