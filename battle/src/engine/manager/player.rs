#[derive(Debug, Clone, Copy, Default)]
pub struct BattlePlayerManager {
    level: i32,
}

impl BattlePlayerManager {
    pub const fn new(level: i32) -> Self {
        Self { level }
    }

    pub const fn level(self) -> i32 {
        self.level
    }
}
