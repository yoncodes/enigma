use super::*;
use sonettobuf::BuffActInfo;

fn recovery_fight() -> Fight {
    Fight {
        version: Some(7),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(1000),
                toughness_value: Some(100),
                toughness_point: Some(1),
                passive_skill: vec![103360263],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(118350001),
                    from_uid: Some(-1),
                    act_info: vec![BuffActInfo {
                        act_id: Some(1102),
                        param: vec![0],
                        str_param: Some(String::new()),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn recovery_catalog() -> SkillEffectCatalog {
    let mut catalog = SkillEffectCatalog::default();
    crate::catalog::BattleCatalog::new(crate::test_support::game_data()).extend_skill_roots(
        &mut catalog,
        [103360263],
        [],
    );
    catalog
}

#[test]
fn scheduled_toughness_recovery_waits_for_the_following_round_start() {
    init_config();
    let fight = recovery_fight();
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = recovery_catalog();
    let mut determinism = RoundDeterminism::default();
    let mut settle = |managers: &mut BattleManagers| {
        run_before_ai_round_start(
            BattleAccess::new(
                managers,
                &pool,
                &catalog,
                &mut determinism,
                TargetContext::default(),
            ),
            2,
            &[],
            None,
        )
        .unwrap();
    };

    for new_stacks in 0..2 {
        assert!(managers.toughness.reduce(-1, 100, true).unwrap().broke);
        settle(&mut managers);
        assert!(managers.toughness.is_broken(-1));
        assert_eq!(managers.buff.buff_id_amount(-1, 109320111), new_stacks);
        assert_eq!(
            managers.buff.snapshot(-1, 20).unwrap().act_info[0].param,
            [1]
        );

        settle(&mut managers);
        assert!(!managers.toughness.is_broken(-1));
        assert_eq!(managers.toughness.get(-1).unwrap().value, 100);
        assert_eq!(managers.buff.buff_id_amount(-1, 109320111), new_stacks + 1);
        assert_eq!(
            managers.buff.snapshot(-1, 20).unwrap().act_info[0].param,
            [0]
        );
    }
}

#[test]
fn unbroken_toughness_timer_counts_down_from_the_defender_start() {
    init_config();
    let fight = recovery_fight();
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = recovery_catalog();
    let mut determinism = RoundDeterminism::default();
    let mut settle = |managers: &mut BattleManagers, defender_start_round| {
        run_before_ai_round_start(
            BattleAccess::new(
                managers,
                &pool,
                &catalog,
                &mut determinism,
                TargetContext {
                    defender_start_round,
                    ..TargetContext::default()
                },
            ),
            2,
            &[],
            None,
        )
        .unwrap();
    };
    let timer = |managers: &BattleManagers| {
        managers.buff.snapshot(-1, 20).unwrap().act_info[0]
            .param
            .clone()
    };

    // Without a defender start count (older layouts) the unbroken timer is left alone.
    settle(&mut managers, None);
    assert_eq!(timer(&managers), [0]);

    // Version 7 sends `1 - n` on the n-th defender start while unbroken.
    settle(&mut managers, Some(1));
    assert_eq!(timer(&managers), [0]);
    settle(&mut managers, Some(5));
    assert_eq!(timer(&managers), [-4]);
    assert!(!managers.toughness.is_broken(-1));
}

#[test]
fn toughness_timer_row_is_added_by_its_first_update() {
    init_config();
    let mut fight = recovery_fight();
    fight.defender.as_mut().unwrap().entitys[0].buffs[0]
        .act_info
        .clear();
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = recovery_catalog();
    let mut determinism = RoundDeterminism::default();
    run_before_ai_round_start(
        BattleAccess::new(
            &mut managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext {
                defender_start_round: Some(2),
                ..TargetContext::default()
            },
        ),
        2,
        &[],
        None,
    )
    .unwrap();

    let info = managers.buff.snapshot(-1, 20).unwrap().act_info;
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].act_id, Some(1102));
    assert_eq!(info[0].param, [-1]);
}
