use serde::{Deserialize, Serialize};

pub const INPUT_LEFT: u8 = 1 << 0;
pub const INPUT_RIGHT: u8 = 1 << 1;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FrameInput {
    pub p0: u8,
    pub p1: u8,
}

impl FrameInput {
    pub fn for_player(self, player: usize) -> u8 {
        match player {
            0 => self.p0,
            1 => self.p1,
            _ => 0,
        }
    }

    pub fn with_player(mut self, player: usize, mask: u8) -> Self {
        match player {
            0 => self.p0 = mask,
            1 => self.p1 = mask,
            _ => {}
        }
        self
    }

    pub fn direction_x(mask: u8) -> i32 {
        let mut dir = 0;
        if mask & INPUT_LEFT != 0 {
            dir -= 1;
        }
        if mask & INPUT_RIGHT != 0 {
            dir += 1;
        }
        dir
    }
}
