use serde::{Deserialize, Serialize};

pub type InputMask = u16;

pub const INPUT_LEFT: InputMask = 1 << 0;
pub const INPUT_RIGHT: InputMask = 1 << 1;
pub const INPUT_UP: InputMask = 1 << 2;
pub const INPUT_DOWN: InputMask = 1 << 3;
pub const INPUT_JUMP: InputMask = 1 << 4;
pub const INPUT_SPIN: InputMask = 1 << 5;
/// Rotate paddle counter-clockwise (arrows / right stick).
pub const INPUT_ANGLE_CCW: InputMask = 1 << 6;
/// Rotate paddle clockwise.
pub const INPUT_ANGLE_CW: InputMask = 1 << 7;
/// Grappleshot charge / fire (keyboard X · pad Y / North · LT).
pub const INPUT_GRAPPLE: InputMask = 1 << 8;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrameInput {
    pub p0: InputMask,
    pub p1: InputMask,
}

impl FrameInput {
    pub fn for_player(self, player: usize) -> InputMask {
        match player {
            0 => self.p0,
            1 => self.p1,
            _ => 0,
        }
    }

    pub fn with_player(mut self, player: usize, mask: InputMask) -> Self {
        match player {
            0 => self.p0 = mask,
            1 => self.p1 = mask,
            _ => {}
        }
        self
    }

    pub fn direction_x(mask: InputMask) -> i32 {
        let mut dir = 0;
        if mask & INPUT_LEFT != 0 {
            dir -= 1;
        }
        if mask & INPUT_RIGHT != 0 {
            dir += 1;
        }
        dir
    }

    pub fn direction_y(mask: InputMask) -> i32 {
        let mut dir = 0;
        if mask & INPUT_UP != 0 {
            dir += 1;
        }
        if mask & INPUT_DOWN != 0 {
            dir -= 1;
        }
        dir
    }

    pub fn angle_dir(mask: InputMask) -> i32 {
        let mut dir = 0;
        if mask & INPUT_ANGLE_CCW != 0 {
            dir -= 1;
        }
        if mask & INPUT_ANGLE_CW != 0 {
            dir += 1;
        }
        dir
    }
}
