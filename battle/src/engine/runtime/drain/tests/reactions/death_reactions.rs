use super::*;

#[test]
fn allied_death_condition_fires_for_the_surviving_owner() {
    crate::test_support::init_config();

    let entity = |uid, passive_skill| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        current_hp: Some(100),
        passive_skill,
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![entity(-1, vec![103374115]), entity(-2, Vec::new())],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    managers.hp.lose(-2, 100, 10);

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: -2,
        }),
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(-1, 103374115), 1);
}

#[test]
fn exact_dead_condition_fires_for_the_defeated_owner() {
    crate::test_support::init_config();

    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![211203],
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    managers.hp.lose(10, 100, -1);

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: -1,
            target_uid: 10,
        }),
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(10, 2112031), 1);
}

#[test]
fn self_death_condition_only_fires_for_the_defeated_owner() {
    crate::test_support::init_config();

    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                attr: Some(HeroAttribute {
                    hp: Some(100),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(100),
                    buff_id: Some(116330001),
                    layer: Some(3),
                    from_uid: Some(-1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    current_hp: Some(100),
                    passive_skill: vec![116330100],
                    attr: Some(HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(-2),
                    team_type: Some(2),
                    current_hp: Some(100),
                    passive_skill: vec![116330100],
                    attr: Some(HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    managers.hp.lose(-1, 100, 10);

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: -1,
        }),
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(10, 116330001), 1);
}

#[test]
fn enemy_death_condition_fires_for_the_observing_side() {
    crate::test_support::init_config();

    let entity = |uid, team_type, passive_skill| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        passive_skill,
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, vec![30581]), entity(11, 1, Vec::new())],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, Vec::new()), entity(-2, 2, Vec::new())],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    managers.hp.lose(-1, 100, 10);

    let result = run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: -1,
        }),
    )
    .unwrap();

    assert_eq!(managers.buff.buff_id_amount(10, 30581), 1);
    assert_eq!(managers.buff.buff_id_amount(11, 30581), 1);

    fn collect_frames(
        frame: &crate::engine::runtime::record::SemanticFrame,
        owners: &mut Vec<(i64, Option<i64>)>,
    ) {
        if let crate::engine::runtime::record::FrameOwner::Skill {
            source_uid,
            skill_id: 30581,
            target_uid,
            ..
        } = frame.owner
        {
            owners.push((source_uid, target_uid));
        }
        for item in &frame.items {
            if let crate::engine::runtime::record::FrameItem::Child(child) = item {
                collect_frames(child, owners);
            }
        }
    }

    let mut owners = Vec::new();
    for frame in &result.frames {
        collect_frames(frame, &mut owners);
    }
    assert_eq!(
        owners,
        vec![(crate::engine::fight::rules::ATTACKER_SIDE_UID, Some(-1))]
    );
}

#[test]
fn ally_death_reaction_targets_only_the_defeated_ally() {
    crate::test_support::init_config();

    let entity = |uid, position, hp, passive_skill| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(2),
        position: Some(position),
        current_hp: Some(hp),
        passive_skill,
        attr: Some(HeroAttribute {
            hp: Some(hp),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![
                entity(-2, 1, 1_000, Vec::new()),
                entity(-3, 2, 1_000, Vec::new()),
                entity(-5, 3, 4_000, vec![2734]),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    managers.hp.lose(-3, 1_000, 10);

    run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::EntityDied(crate::engine::event::payload::EntityDiedEvent {
            source_uid: 10,
            target_uid: -3,
        }),
    )
    .unwrap();

    assert_eq!(managers.hp.current(-3), 150);
    assert_eq!(managers.buff.buff_id_amount(-3, 27341), 1);
    assert_eq!(managers.buff.buff_id_amount(-3, 27342), 1);
    for (uid, hp) in [(-2, 1_000), (-5, 4_000)] {
        assert_eq!(managers.hp.current(uid), hp);
        assert_eq!(managers.buff.buff_id_amount(uid, 27341), 0);
        assert_eq!(managers.buff.buff_id_amount(uid, 27342), 0);
    }
}
