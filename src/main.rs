use bevy::prelude::*;
use game::*;

mod components;
mod game;
mod input;
mod netcode;


fn main() {
    let mut app = App::new();
    build_app(&mut app);
    app.run();
}