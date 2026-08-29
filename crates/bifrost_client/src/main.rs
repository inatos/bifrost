mod app;
mod args;
mod bot_mode;
mod hud;
mod input_focus;
mod interp;
mod juice;
mod local_input;
mod net_mode;
mod online_mode;
mod render;
mod session_boot;
mod state;

use app::run;

fn main() {
    #[cfg(target_arch = "wasm32")]
    console_error_panic_hook::set_once();
    run();
}
