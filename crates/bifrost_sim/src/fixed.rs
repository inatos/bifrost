//! Fixed-point helpers (1 unit = 1/FP_SCALE world units).

pub const FP_SCALE: i32 = 1000;

#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct Vec2 {
    pub x: i32,
    pub y: i32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0, y: 0 };

    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub fn from_f(x: f32, y: f32) -> Self {
        Self {
            x: (x * FP_SCALE as f32) as i32,
            y: (y * FP_SCALE as f32) as i32,
        }
    }

    pub fn to_f(self) -> (f32, f32) {
        (
            self.x as f32 / FP_SCALE as f32,
            self.y as f32 / FP_SCALE as f32,
        )
    }

    pub fn add(self, other: Self) -> Self {
        Self {
            x: self.x.saturating_add(other.x),
            y: self.y.saturating_add(other.y),
        }
    }

    pub fn sub(self, other: Self) -> Self {
        Self {
            x: self.x.saturating_sub(other.x),
            y: self.y.saturating_sub(other.y),
        }
    }

    pub fn scale(self, factor: i32, divisor: i32) -> Self {
        Self {
            x: ((self.x as i64 * factor as i64) / divisor as i64) as i32,
            y: ((self.y as i64 * factor as i64) / divisor as i64) as i32,
        }
    }

    pub fn len_sq(self) -> i64 {
        (self.x as i64) * (self.x as i64) + (self.y as i64) * (self.y as i64)
    }

    pub fn normalize(self) -> Self {
        let len = isqrt(self.len_sq());
        if len == 0 {
            return Self::ZERO;
        }
        Self {
            x: ((self.x as i64 * FP_SCALE as i64) / len) as i32,
            y: ((self.y as i64 * FP_SCALE as i64) / len) as i32,
        }
    }
}

pub fn isqrt(n: i64) -> i64 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}
