use crate::GameDB;

impl GameDB {
    pub fn toughness_passive_skill(&self, toughness_skill: i32) -> Option<i32> {
        self.toughnessskill
            .iter()
            .find(|row| row.toughnessskill == toughness_skill)?
            .passive_skill
            .parse()
            .ok()
    }
}
