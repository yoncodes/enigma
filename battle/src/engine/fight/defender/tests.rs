use super::*;

#[test]
fn unseeded_boss_uses_instance_job_scaling() {
    crate::test_support::init_config();

    let boss = Defender::build_monster_with_uid(30111001, -1, 1, 2).unwrap();
    assert_eq!(
        boss,
        Defender::build_monster(
            crate::catalog::BattleCatalog::new(crate::test_support::game_data()),
            30111001,
            -1,
            1,
            2,
        )
        .unwrap()
    );
    let attr = boss.attr.unwrap();

    assert_eq!(boss.current_hp, Some(67_680));
    assert_eq!(attr.hp, Some(67_680));
    assert_eq!(attr.attack, Some(1_696));
    assert_eq!(attr.defense, Some(1_000));
    assert_eq!(attr.mdefense, Some(736));
    assert_eq!(attr.technic, Some(210));
}

#[test]
fn monster_toughness_skill_contributes_its_configured_passive() {
    crate::test_support::init_config();

    let entity = Defender::build_monster_with_uid(1_163_857_113, -1, 1, 2).unwrap();

    assert!(entity.passive_skill.contains(&116_362_200));
}

#[test]
fn monster_break_state_exists_only_with_configured_toughness() {
    crate::test_support::init_config();

    let without_toughness = Defender::build_monster_with_uid(4_030_703, -1, 1, 2).unwrap();
    assert_eq!(without_toughness.toughness_value, None);
    assert_eq!(without_toughness.toughness_point, None);
    assert_eq!(without_toughness.is_broken, None);

    let with_toughness = Defender::build_monster_with_uid(1_163_857_113, -2, 1, 2).unwrap();
    assert!(with_toughness.toughness_value.is_some());
    assert!(with_toughness.toughness_point.is_some());
    assert_eq!(with_toughness.is_broken, Some(false));
}

#[test]
fn monster_unique_skill_level_is_not_serialized_as_a_hero_ex_rank() {
    crate::test_support::init_config();

    let monster = Defender::build_monster_with_uid(4_030_703, -3, 1, 2).unwrap();

    assert_eq!(monster.ex_skill, Some(40_231_331));
    assert_eq!(monster.ex_skill_level, Some(0));
}

#[test]
fn monster_moxie_maximum_uses_its_skill_template() {
    crate::test_support::init_config();

    let mut monster = Defender::build_monster_with_uid(109_360_002, -1, 1, 2).unwrap();

    assert_eq!(monster.ex_point_max, Some(2));

    let without_moxie = Defender::build_monster_with_uid(8_101, -2, 1, 2).unwrap();
    assert_eq!(without_moxie.ex_point_max, Some(0));

    monster.ex_point = Some(2);
    let mut manager = crate::engine::manager::ex_point::ExPointManager::default();
    manager.register(&monster);

    assert!(manager.is_full(-1));
}

#[test]
fn monster_starts_with_configured_moxie() {
    crate::test_support::init_config();

    let setup = Defender::get(1001, 2).unwrap();
    let monster = setup
        .team
        .entitys
        .iter()
        .find(|entity| entity.model_id == Some(100104))
        .unwrap();

    assert_eq!(monster.ex_point, Some(5));
}

#[test]
fn monster_preserves_all_configured_power_resources() {
    crate::test_support::init_config();

    let monster = Defender::build_monster_with_uid(109_380_001, -1, 1, 2).unwrap();
    let powers = monster
        .power_infos
        .iter()
        .map(|power| (power.power_id, power.num, power.max))
        .collect::<Vec<_>>();

    assert_eq!(
        powers,
        vec![(Some(10), Some(0), Some(100)), (Some(9), Some(0), Some(4))]
    );
}

#[test]
fn tower_supporter_uses_reserved_normal_uid_space() {
    crate::test_support::init_config();

    let setup = Defender::get(9000303, 1).unwrap();

    assert_eq!(
        setup
            .team
            .entitys
            .iter()
            .filter_map(|entity| entity.uid)
            .collect::<Vec<_>>(),
        vec![-2, -3, -4]
    );
    assert_eq!(setup.attacker_sp_entitys[0].uid, Some(-6));
    assert_eq!(setup.attacker_sp_entitys[0].model_id, Some(900030304));
}
