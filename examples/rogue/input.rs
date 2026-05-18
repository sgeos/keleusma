//! Keyboard-to-command translation. Centralised so the main
//! loop does not branch on raw key codes and the keybind table
//! is easy to inspect.

use sdl3::keyboard::Keycode;

/// High-level commands the game loop dispatches on.
#[derive(Clone, Copy, Debug)]
pub enum Command {
    Move(i32, i32),
    Descend,
    Quaff,
    Read,
    Reload,
    Quit,
}

/// Translate a key event into a game command, or return `None`
/// if the key is not bound. Arrow keys and the vi-style movement
/// keys both work for movement. Greater-than triggers stairs
/// descent. `Q` quaffs the held potion; `R` reads the held
/// scroll. Escape quits.
pub fn translate(keycode: Keycode) -> Option<Command> {
    match keycode {
        Keycode::Escape => Some(Command::Quit),

        // Cardinal movement.
        Keycode::Up | Keycode::K => Some(Command::Move(0, -1)),
        Keycode::Down | Keycode::J => Some(Command::Move(0, 1)),
        Keycode::Left | Keycode::H => Some(Command::Move(-1, 0)),
        Keycode::Right | Keycode::L => Some(Command::Move(1, 0)),

        // Diagonals.
        Keycode::Y => Some(Command::Move(-1, -1)),
        Keycode::U => Some(Command::Move(1, -1)),
        Keycode::B => Some(Command::Move(-1, 1)),
        Keycode::N => Some(Command::Move(1, 1)),

        // Wait in place.
        Keycode::Period | Keycode::Space => Some(Command::Move(0, 0)),

        // Stairs descent and item use.
        Keycode::Greater => Some(Command::Descend),
        Keycode::Q => Some(Command::Quaff),
        Keycode::R => Some(Command::Read),

        // Hot reload the Keleusma scripts from disk.
        Keycode::F5 => Some(Command::Reload),

        _ => None,
    }
}
