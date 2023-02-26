use bevy::prelude::*;
use bevy_ggrs::ggrs::PlayerHandle;
use bitflags::bitflags;


bitflags! {
    struct  PlayerInput: u8 {
        const UP = 1 << 0;
        const DOWN = 1 << 1;
        const LEFT = 1 << 2;
        const RIGHT = 1 << 3;
    }
}

/// Handles player input
pub fn input(_: In<PlayerHandle>, keys: Res<Input<KeyCode>>) -> u8 {
    let mut input = 0u8;

    if keys.any_pressed([KeyCode::Up, KeyCode::W]) {
        input |= PlayerInput::UP.bits;
    }
    if keys.any_pressed([KeyCode::Down, KeyCode::S]) {
        input |= PlayerInput::DOWN.bits;
    }
    if keys.any_pressed([KeyCode::Left, KeyCode::A]) {
        input |= PlayerInput::LEFT.bits;
    }
    if keys.any_pressed([KeyCode::Right, KeyCode::D]) {
        input |= PlayerInput::RIGHT.bits;
    }

    return input;
}

pub fn direction(input: u8) -> Vec2 {
    let mut direction = Vec2::ZERO;

    if input & PlayerInput::UP.bits != 0 {
        direction.y += 1.0;
    }
    if input & PlayerInput::DOWN.bits != 0 {
        direction.y -= 1.0;
    }
    if input & PlayerInput::LEFT.bits != 0 {
        direction.x -= 1.0;
    }
    if input & PlayerInput::RIGHT.bits != 0 {
        direction.x += 1.0;
    }

    return direction;
}