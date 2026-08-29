//! Local player input: keyboard, gamepad, or mouse (active device wins).

use bevy::input::gamepad::{Gamepad, GamepadButton};
use bevy::prelude::*;
use bifrost_sim::{
    INPUT_ANGLE_CCW, INPUT_ANGLE_CW, INPUT_DOWN, INPUT_JUMP, INPUT_LEFT, INPUT_RIGHT, INPUT_SPIN,
    INPUT_UP, FP_SCALE, PADDLE_SPEED, PADDLE_W,
};

use crate::input_focus::{InputDevice, InputFocus};

const STICK_THRESHOLD: f32 = 0.28;
/// Stop chase inside this band. Must be ≥ one frame of paddle travel or digital
/// L/R/U/D overshoots the cursor and oscillates (figure-8 / side-flip).
const MOUSE_DEADZONE_X: i32 = PADDLE_SPEED * 2;
const MOUSE_DEADZONE_Y: i32 = PADDLE_SPEED * 2;

pub fn local_input_mask(
    keys: &ButtonInput<KeyCode>,
    mouse: &ButtonInput<MouseButton>,
    gamepads: &Query<&Gamepad>,
    windows: &Query<&Window>,
    camera_q: &Query<(&Camera, &GlobalTransform, &Transform)>,
    paddle_x: i32,
    paddle_y: i32,
    focus: &mut InputFocus,
) -> u8 {
    let kb = keyboard_move_mask(keys);
    let mut angle = angle_mask(keys, gamepads) | embed_angle_mask();
    let (embed_dirs, embed_south, embed_west) = embed_pad_state();
    let (embed_kb, embed_jump, embed_spin) = embed_keys_state();
    let pad = gamepad_move_mask(gamepads) | embed_dirs | embed_kb;
    let mut jump = jump_mask(keys, gamepads, focus) | embed_jump;
    let mut spin = spin_mask(keys, gamepads, focus) | embed_spin;
    if embed_south {
        focus.note_gamepad();
        jump |= INPUT_JUMP;
    }
    if embed_west {
        focus.note_gamepad();
        spin |= INPUT_SPIN;
    }
    // Mouse: LMB = spin charge, RMB = snapback stance (angle toward cursor).
    if mouse.pressed(MouseButton::Left) {
        focus.note_mouse_at(0.0, 0.0);
        spin |= INPUT_SPIN;
    }
    if mouse.pressed(MouseButton::Right) {
        focus.note_mouse_at(0.0, 0.0);
        angle |= mouse_snapback_angle(windows, camera_q, paddle_x, paddle_y);
    }
    // While RMB snapback stance: feed move bits toward cursor so release can
    // invert to a full-360° beam (opposite stick/cursor release toward center).
    let mouse_aim = if mouse.pressed(MouseButton::Right) {
        mouse_aim_dirs(windows, camera_q, paddle_x, paddle_y)
    } else {
        0
    };
    poll_cursor_focus(windows, camera_q, focus);

    let extras = jump | spin | angle | mouse_aim;
    if kb != 0 {
        focus.note_keyboard();
        return kb | extras;
    }
    if pad != 0 || embed_south || embed_west || embed_kb != 0 || embed_jump != 0 || embed_spin != 0 {
        focus.note_gamepad();
        return pad | extras;
    }
    if angle != 0 || mouse.pressed(MouseButton::Left) || mouse.pressed(MouseButton::Right) {
        return extras;
    }

    match focus.device {
        InputDevice::Mouse => {
            mouse_directions(windows, camera_q, paddle_x, paddle_y, focus) | extras
        }
        InputDevice::Gamepad | InputDevice::Keyboard => extras,
    }
}

fn jump_mask(
    keys: &ButtonInput<KeyCode>,
    gamepads: &Query<&Gamepad>,
    focus: &mut InputFocus,
) -> u8 {
    // Held level bit — sim edges takeoff/pound; hold enables float.
    if keys.pressed(KeyCode::Space) {
        focus.note_keyboard();
        return INPUT_JUMP;
    }
    for gamepad in gamepads.iter() {
        if gamepad.pressed(GamepadButton::South) {
            focus.note_gamepad();
            return INPUT_JUMP;
        }
    }
    0
}

fn spin_mask(
    keys: &ButtonInput<KeyCode>,
    gamepads: &Query<&Gamepad>,
    focus: &mut InputFocus,
) -> u8 {
    if keys.pressed(KeyCode::KeyX) {
        focus.note_keyboard();
        return INPUT_SPIN;
    }
    for gamepad in gamepads.iter() {
        if gamepad.pressed(GamepadButton::West)
            || gamepad.pressed(GamepadButton::RightTrigger)
            || gamepad.pressed(GamepadButton::RightTrigger2)
        {
            focus.note_gamepad();
            return INPUT_SPIN;
        }
    }
    0
}

/// WASD only — arrows are reserved for paddle angle.
fn keyboard_move_mask(keys: &ButtonInput<KeyCode>) -> u8 {
    let mut mask = 0u8;
    if keys.pressed(KeyCode::KeyA) {
        mask |= INPUT_LEFT;
    }
    if keys.pressed(KeyCode::KeyD) {
        mask |= INPUT_RIGHT;
    }
    if keys.pressed(KeyCode::KeyW) {
        mask |= INPUT_UP;
    }
    if keys.pressed(KeyCode::KeyS) {
        mask |= INPUT_DOWN;
    }
    mask
}

fn angle_mask(keys: &ButtonInput<KeyCode>, gamepads: &Query<&Gamepad>) -> u8 {
    let invert = embed_invert_angle();
    let mut mask = 0u8;
    let mut ccw = keys.pressed(KeyCode::ArrowLeft) || keys.pressed(KeyCode::ArrowUp);
    let mut cw = keys.pressed(KeyCode::ArrowRight) || keys.pressed(KeyCode::ArrowDown);
    // Cardinals travel with angle bits so the sim can latch 2D snap aim
    // (R-stick at 6:00 → fire at 12:00). Movement is suppressed while winding.
    if keys.pressed(KeyCode::ArrowLeft) {
        mask |= INPUT_LEFT;
    }
    if keys.pressed(KeyCode::ArrowRight) {
        mask |= INPUT_RIGHT;
    }
    if keys.pressed(KeyCode::ArrowUp) {
        mask |= INPUT_UP;
    }
    if keys.pressed(KeyCode::ArrowDown) {
        mask |= INPUT_DOWN;
    }
    for gamepad in gamepads.iter() {
        let stick = gamepad.right_stick();
        if stick.x < -STICK_THRESHOLD {
            ccw = true;
            mask |= INPUT_LEFT;
        }
        if stick.x > STICK_THRESHOLD {
            cw = true;
            mask |= INPUT_RIGHT;
        }
        if stick.y > STICK_THRESHOLD {
            ccw = true;
            mask |= INPUT_UP;
        }
        if stick.y < -STICK_THRESHOLD {
            cw = true;
            mask |= INPUT_DOWN;
        }
        break;
    }
    if invert {
        std::mem::swap(&mut ccw, &mut cw);
    }
    if ccw {
        mask |= INPUT_ANGLE_CCW;
    }
    if cw {
        mask |= INPUT_ANGLE_CW;
    }
    mask
}

fn embed_invert_angle() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        web_sys::window()
            .and_then(|w| js_sys::Reflect::get(&w, &"__bifrostInvertAngle".into()).ok())
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        false
    }
}

fn gamepad_move_mask(gamepads: &Query<&Gamepad>) -> u8 {
    let mut mask = 0u8;
    for gamepad in gamepads.iter() {
        let stick = gamepad.left_stick();
        if stick.x < -STICK_THRESHOLD {
            mask |= INPUT_LEFT;
        }
        if stick.x > STICK_THRESHOLD {
            mask |= INPUT_RIGHT;
        }
        if stick.y > STICK_THRESHOLD {
            mask |= INPUT_UP;
        }
        if stick.y < -STICK_THRESHOLD {
            mask |= INPUT_DOWN;
        }
        if gamepad.pressed(GamepadButton::DPadLeft) {
            mask |= INPUT_LEFT;
        }
        if gamepad.pressed(GamepadButton::DPadRight) {
            mask |= INPUT_RIGHT;
        }
        if gamepad.pressed(GamepadButton::DPadUp) {
            mask |= INPUT_UP;
        }
        if gamepad.pressed(GamepadButton::DPadDown) {
            mask |= INPUT_DOWN;
        }
        break;
    }
    mask
}

/// Lab parent posts `{ type:'bifrost-pad', lx, ly, rx, ry, south, west }`.
fn embed_pad_state() -> (u8, bool, bool) {
    #[cfg(target_arch = "wasm32")]
    {
        let Some((lx, ly, south, west)) = read_embed_pad() else {
            return (0, false, false);
        };
        let mut mask = 0u8;
        if lx < -STICK_THRESHOLD {
            mask |= INPUT_LEFT;
        }
        if lx > STICK_THRESHOLD {
            mask |= INPUT_RIGHT;
        }
        if ly < -STICK_THRESHOLD {
            mask |= INPUT_UP;
        }
        if ly > STICK_THRESHOLD {
            mask |= INPUT_DOWN;
        }
        (mask, south, west)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        (0, false, false)
    }
}

fn embed_angle_mask() -> u8 {
    #[cfg(target_arch = "wasm32")]
    {
        let Some((rx, ry)) = read_embed_rstick() else {
            return 0;
        };
        // Embed pad Y matches left-stick embed (y- = up). Include aim cardinals.
        let mut mask = 0u8;
        if rx < -STICK_THRESHOLD {
            mask |= INPUT_ANGLE_CCW | INPUT_LEFT;
        }
        if rx > STICK_THRESHOLD {
            mask |= INPUT_ANGLE_CW | INPUT_RIGHT;
        }
        if ry < -STICK_THRESHOLD {
            mask |= INPUT_ANGLE_CCW | INPUT_UP;
        }
        if ry > STICK_THRESHOLD {
            mask |= INPUT_ANGLE_CW | INPUT_DOWN;
        }
        mask
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        0
    }
}

/// RMB aim: direction bits from paddle toward cursor (for Snapback release).
fn mouse_aim_dirs(
    windows: &Query<&Window>,
    camera_q: &Query<(&Camera, &GlobalTransform, &Transform)>,
    paddle_x: i32,
    paddle_y: i32,
) -> u8 {
    let Some((wx, wy)) = cursor_world_fp(windows, camera_q) else {
        return 0;
    };
    let dx = wx - paddle_x;
    let dy = wy - paddle_y;
    let thresh = PADDLE_W / 8;
    let mut mask = 0u8;
    if dx > thresh {
        mask |= INPUT_RIGHT;
    } else if dx < -thresh {
        mask |= INPUT_LEFT;
    }
    if dy > thresh {
        mask |= INPUT_UP;
    } else if dy < -thresh {
        mask |= INPUT_DOWN;
    }
    mask
}

/// RMB snapback: wind angle toward the cursor (full 2D heading).
fn mouse_snapback_angle(
    windows: &Query<&Window>,
    camera_q: &Query<(&Camera, &GlobalTransform, &Transform)>,
    paddle_x: i32,
    paddle_y: i32,
) -> u8 {
    let Some((wx, wy)) = cursor_world_fp(windows, camera_q) else {
        return INPUT_ANGLE_CW;
    };
    let dx = wx - paddle_x;
    let dy = wy - paddle_y;
    // Prefer CW when aiming to the right of the paddle facing; CCW left.
    // Also use vertical: aim above → CCW for typical P0 wind convention.
    if dx.abs() >= dy.abs() {
        if dx >= 0 {
            INPUT_ANGLE_CW
        } else {
            INPUT_ANGLE_CCW
        }
    } else if dy >= 0 {
        INPUT_ANGLE_CCW
    } else {
        INPUT_ANGLE_CW
    }
}

/// Map cursor to fixed-point world, ignoring camera shake translation (juice).
fn cursor_world_fp(
    windows: &Query<&Window>,
    camera_q: &Query<(&Camera, &GlobalTransform, &Transform)>,
) -> Option<(i32, i32)> {
    let window = windows.single().ok()?;
    let (camera, _gt, transform) = camera_q.single().ok()?;
    let cursor = window.cursor_position()?;
    let mut unshaken = *transform;
    unshaken.translation.x = 0.0;
    unshaken.translation.y = 0.0;
    let gt = GlobalTransform::from(unshaken);
    let world = camera.viewport_to_world_2d(&gt, cursor).ok()?;
    Some((
        (world.x * FP_SCALE as f32) as i32,
        (world.y * FP_SCALE as f32) as i32,
    ))
}

#[cfg(target_arch = "wasm32")]
fn read_embed_pad() -> Option<(f32, f32, bool, bool)> {
    let window = web_sys::window()?;
    let hook = js_sys::Reflect::get(&window, &"__bifrostPad".into()).ok()?;
    if hook.is_undefined() || hook.is_null() {
        return None;
    }
    let lx = js_sys::Reflect::get(&hook, &"lx".into())
        .ok()?
        .as_f64()? as f32;
    let ly = js_sys::Reflect::get(&hook, &"ly".into())
        .ok()?
        .as_f64()? as f32;
    let south = js_sys::Reflect::get(&hook, &"south".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let west = js_sys::Reflect::get(&hook, &"west".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let spin = js_sys::Reflect::get(&hook, &"spin".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let spin_held = west || spin;
    if south {
        let _ = js_sys::Reflect::set(&hook, &"south".into(), &wasm_bindgen::JsValue::FALSE);
    }
    Some((lx, ly, south, spin_held))
}

#[cfg(target_arch = "wasm32")]
fn read_embed_rstick() -> Option<(f32, f32)> {
    let window = web_sys::window()?;
    let hook = js_sys::Reflect::get(&window, &"__bifrostPad".into()).ok()?;
    if hook.is_undefined() || hook.is_null() {
        return None;
    }
    let rx = js_sys::Reflect::get(&hook, &"rx".into())
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;
    let ry = js_sys::Reflect::get(&hook, &"ry".into())
        .ok()
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0) as f32;
    Some((rx, ry))
}

/// Parent frame posts `{ type:'bifrost-key', code, down }` for cross-origin embed focus.
#[cfg(target_arch = "wasm32")]
fn embed_keys_state() -> (u8, u8, u8) {
    let Some(window) = web_sys::window() else {
        return (0, 0, 0);
    };
    let Ok(hook) = js_sys::Reflect::get(&window, &"__bifrostKeys".into()) else {
        return (0, 0, 0);
    };
    if hook.is_undefined() || hook.is_null() {
        return (0, 0, 0);
    }
    let down = |code: &str| -> bool {
        js_sys::Reflect::get(&hook, &code.into())
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    };
    let mut mask = 0u8;
    // WASD move only from embed keys (arrows → angle).
    if down("KeyA") {
        mask |= INPUT_LEFT;
    }
    if down("KeyD") {
        mask |= INPUT_RIGHT;
    }
    if down("KeyW") {
        mask |= INPUT_UP;
    }
    if down("KeyS") {
        mask |= INPUT_DOWN;
    }
    let mut jump = 0u8;
    if down("Space")
        || js_sys::Reflect::get(&window, &"__bifrostKeyJump".into())
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    {
        jump |= INPUT_JUMP;
    }
    let spin = if down("KeyX") { INPUT_SPIN } else { 0 };
    // Angle + aim cardinals from arrow keys via embed.
    if down("ArrowLeft") {
        mask |= INPUT_ANGLE_CCW | INPUT_LEFT;
    }
    if down("ArrowRight") {
        mask |= INPUT_ANGLE_CW | INPUT_RIGHT;
    }
    if down("ArrowUp") {
        mask |= INPUT_ANGLE_CCW | INPUT_UP;
    }
    if down("ArrowDown") {
        mask |= INPUT_ANGLE_CW | INPUT_DOWN;
    }
    (mask, jump, spin)
}

#[cfg(not(target_arch = "wasm32"))]
fn embed_keys_state() -> (u8, u8, u8) {
    (0, 0, 0)
}

fn poll_cursor_focus(
    windows: &Query<&Window>,
    _camera_q: &Query<(&Camera, &GlobalTransform, &Transform)>,
    focus: &mut InputFocus,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        return;
    };
    focus.note_mouse_at(cursor.x, cursor.y);
}

fn mouse_directions(
    windows: &Query<&Window>,
    camera_q: &Query<(&Camera, &GlobalTransform, &Transform)>,
    paddle_x: i32,
    paddle_y: i32,
    focus: &mut InputFocus,
) -> u8 {
    let Some((target_x, target_y)) = cursor_world_fp(windows, camera_q) else {
        return focus.mouse_move_sticky & (INPUT_LEFT | INPUT_RIGHT | INPUT_UP | INPUT_DOWN);
    };
    let mut mask = 0u8;
    let dx = target_x - paddle_x;
    let dy = target_y - paddle_y;
    // Inside the deadzone: stop. Never keep thrusting through the cursor (the old
    // sticky band did that and caused side-to-side / figure-8 chase).
    if dx > MOUSE_DEADZONE_X {
        mask |= INPUT_RIGHT;
    } else if dx < -MOUSE_DEADZONE_X {
        mask |= INPUT_LEFT;
    }
    if dy > MOUSE_DEADZONE_Y {
        mask |= INPUT_UP;
    } else if dy < -MOUSE_DEADZONE_Y {
        mask |= INPUT_DOWN;
    }
    // Remember last move for cursor-lost fallback only — not for mid-chase thrust.
    focus.mouse_move_sticky = mask & (INPUT_LEFT | INPUT_RIGHT | INPUT_UP | INPUT_DOWN);
    mask
}
