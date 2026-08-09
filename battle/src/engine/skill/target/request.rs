pub const SOURCE_TARGET_CODE: i32 = 103;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TargetRequest {
    pub code: i32,
    pub raw: Vec<i32>,
}

impl TargetRequest {
    pub fn self_only() -> Self {
        Self {
            code: 0,
            raw: Vec::new(),
        }
    }
}

pub fn target_count(db: &config::GameDB, code: i32) -> i32 {
    crate::catalog::target_count(db, code)
}

pub fn damage_target_count_kind(db: &config::GameDB, code: i32) -> i32 {
    crate::catalog::damage_target_count_kind(db, code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_target_count_distinguishes_single_and_mass_attacks() {
        crate::test_support::init_config();

        let db = crate::test_support::game_data();
        let catalog = crate::catalog::BattleCatalog::new(db);
        assert_eq!(damage_target_count_kind(db, 1), 1);
        assert_eq!(damage_target_count_kind(db, 201), 2);
        assert_eq!(damage_target_count_kind(db, 202), 2);
        assert_eq!(damage_target_count_kind(db, i32::MAX), 0);
        assert_eq!(
            damage_target_count_kind(db, 1),
            catalog.damage_target_count_kind(1)
        );
        assert_eq!(target_count(db, 201), catalog.target_count(201));
    }
}
