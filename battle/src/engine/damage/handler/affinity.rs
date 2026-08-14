use crate::engine::skill::target::TargetEntity;

pub(super) fn regular_multiplier(source_bonus: i32, target_reduction: i32) -> i32 {
    (1000 + source_bonus - target_reduction).max(300)
}

pub(super) fn critical_technique_bonus(
    catalog: crate::catalog::BattleCatalog,
    entity: &TargetEntity,
    target_level: i32,
    fight_const_id: i32,
) -> i32 {
    technique_bonus(
        entity.technic,
        target_level,
        catalog.fight_const_value(fight_const_id),
        catalog.fight_const_value(13),
        catalog.fight_const_value(14),
    )
}

pub(super) fn technique_bonus(
    technic: i32,
    target_level: i32,
    ratio: i32,
    correct: i32,
    level_ratio: i32,
) -> i32 {
    let denominator = correct + target_level * level_ratio;
    if denominator <= 0 {
        0
    } else {
        technic * ratio / denominator
    }
}

pub(crate) fn restrains(catalog: crate::catalog::BattleCatalog, source: i32, target: i32) -> bool {
    catalog.career_multiplier(source, target) > 1000
}

pub(crate) fn restrains_target(
    catalog: crate::catalog::BattleCatalog,
    source: i32,
    target: &TargetEntity,
) -> bool {
    target.weak_careers.contains(&source) || restrains(catalog, source, target.career)
}

pub(crate) fn restrains_target_either(
    catalog: crate::catalog::BattleCatalog,
    source: i32,
    additional_source: Option<i32>,
    target: &TargetEntity,
) -> bool {
    restrains_target(catalog, source, target)
        || additional_source.is_some_and(|source| restrains_target(catalog, source, target))
}

pub(super) fn career_multiplier_against(
    catalog: crate::catalog::BattleCatalog,
    source: i32,
    target: &TargetEntity,
) -> i32 {
    if target.weak_careers.contains(&source) {
        catalog.strongest_career_multiplier(source)
    } else {
        catalog.career_multiplier(source, target.career)
    }
}

pub(super) fn career_multiplier_against_either(
    catalog: crate::catalog::BattleCatalog,
    source: i32,
    additional_source: Option<i32>,
    target: &TargetEntity,
) -> i32 {
    additional_source
        .map(|source| career_multiplier_against(catalog, source, target))
        .unwrap_or_default()
        .max(career_multiplier_against(catalog, source, target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_weakness_uses_the_standard_stronger_afflatus_multiplier() {
        crate::test_support::init_config();
        let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
        let target = TargetEntity::from_fight_entity(&sonettobuf::FightEntityInfo {
            uid: Some(-1),
            current_hp: Some(1),
            career: Some(8),
            weak_careers: vec![1, 2],
            ..Default::default()
        })
        .unwrap();

        assert!(restrains_target(catalog, 1, &target));
        assert_eq!(career_multiplier_against(catalog, 1, &target), 1300);
        assert!(!restrains_target(catalog, 3, &target));
        assert_eq!(career_multiplier_against(catalog, 3, &target), 1000);
    }

    #[test]
    fn additional_affinity_uses_the_strongest_multiplier_for_each_target() {
        crate::test_support::init_config();
        let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
        let target = TargetEntity::from_fight_entity(&sonettobuf::FightEntityInfo {
            uid: Some(-1),
            current_hp: Some(1),
            career: Some(8),
            weak_careers: vec![2],
            ..Default::default()
        })
        .unwrap();

        assert!(!restrains_target(catalog, 1, &target));
        assert!(restrains_target_either(catalog, 1, Some(2), &target));
        assert_eq!(
            career_multiplier_against_either(catalog, 1, Some(2), &target),
            1300
        );
    }
}
