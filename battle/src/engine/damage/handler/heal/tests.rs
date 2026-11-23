use super::*;

#[test]
fn healing_modifiers_settle_before_the_final_multiplier() {
    let mut managers = BattleManagers::default();

    assert_eq!(
        modified_fraction(
            733,
            1,
            10,
            11,
            &managers,
            super::super::critical::CriticalHealMultiplier::IDENTITY,
        ),
        733
    );
    let boosted = super::super::critical::CriticalHealMultiplier::from_permille(1_200);
    assert_eq!(modified_fraction(733, 1, 10, 11, &managers, boosted), 879);

    managers.attribute.override_sp(
        10,
        &sonettobuf::HeroSpAttribute {
            heal: Some(1),
            ..Default::default()
        },
    );
    assert_eq!(modified_fraction(733, 1, 10, 11, &managers, boosted), 879);
}

#[test]
fn healing_done_includes_the_max_hp_derived_bonus() {
    crate::test_support::init_config();
    let managers = BattleManagers::seeded(&sonettobuf::Fight {
        attacker: Some(sonettobuf::FightTeam {
            entitys: vec![sonettobuf::FightEntityInfo {
                uid: Some(1),
                current_hp: Some(10_000),
                attr: Some(sonettobuf::HeroAttribute {
                    hp: Some(10_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(2),
                    buff_id: Some(30870302),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    assert_eq!(modified(1_000, 1, 1, &managers, &[]), 1_700);
}
