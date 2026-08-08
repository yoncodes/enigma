use super::*;

#[test]
fn disperse_one_compiles_status_arguments_into_one_manager_command() {
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(30003, "Disperse1"),
        vec![1, 3, 5],
        vec!["1".into(), "3".into(), "5".into()],
    );

    let Some(commands) = dispel_commands(-1, &behavior) else {
        panic!("expected a status dispel command");
    };
    let [BuffCommand::Dispel(command)] = commands.as_slice() else {
        panic!("expected a status dispel command");
    };
    assert_eq!(command.target_uid, -1);
    assert_eq!(command.count, 0);
    assert_eq!(
        command.statuses,
        vec![
            BuffStatus::StatsUp,
            BuffStatus::Counter,
            BuffStatus::PositiveStatus,
        ]
    );
    assert!(command.origin.key.matches(30003, "Disperse1"));

    let definition = registry::find_key(30003, "Disperse1").unwrap();
    assert!(
        definition
            .supports
            .is_some_and(|supports| supports(&behavior))
    );
    let unsupported = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(30003, "Disperse1"),
        vec![99],
        vec!["99".into()],
    );
    assert!(
        !definition
            .supports
            .is_some_and(|supports| supports(&unsupported))
    );
}

#[test]
fn purify_all_uses_every_argument_as_a_removable_status() {
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(20003, "Purify1"),
        Vec::new(),
        vec!["2,4,6".into()],
    );

    let definition = crate::engine::skill::behavior::registry::find(&behavior).unwrap();
    assert!(
        definition
            .supports
            .is_some_and(|supports| supports(&behavior))
    );

    let Some(commands) = dispel_commands(11, &behavior) else {
        panic!("expected a purify command");
    };
    let [BuffCommand::Dispel(command)] = commands.as_slice() else {
        panic!("expected a purify command");
    };
    assert_eq!(command.target_uid, 11);
    assert_eq!(command.count, 0);
    assert_eq!(
        command.statuses,
        vec![
            BuffStatus::StatsDown,
            BuffStatus::Control,
            BuffStatus::NegativeStatus,
        ]
    );
    assert!(command.origin.key.matches(20003, "Purify1"));
}

#[test]
fn disperse_unknown_status_arguments_are_exact_buff_ids() {
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(30009, "Disperse2"),
        vec![9290109],
        vec!["9290109".into()],
    );

    let commands = dispel_commands(11, &behavior).unwrap();

    assert!(matches!(
        commands.as_slice(),
        [BuffCommand::Remove(BuffRemove {
            target_uid: 11,
            selector: BuffRemoveSelector::ExactId(9290109),
            ..
        })]
    ));
    let BuffCommand::Remove(command) = &commands[0] else {
        unreachable!();
    };
    assert!(command.origin.key.matches(30009, "Disperse2"));
}

#[test]
fn disperse_configured_buff_keeps_its_exact_registry_identity() {
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(30004, "Disperse2"),
        vec![530000111],
        vec!["530000111".into()],
    );

    let definition = registry::find(&behavior).unwrap();
    assert!(
        definition
            .supports
            .is_some_and(|supports| supports(&behavior))
    );
    let commands = dispel_commands(11, &behavior).unwrap();
    let [BuffCommand::Remove(command)] = commands.as_slice() else {
        panic!("expected one exact buff removal command");
    };
    assert_eq!(command.target_uid, 11);
    assert_eq!(command.selector, BuffRemoveSelector::ExactId(530000111));
    assert!(command.origin.key.matches(30004, "Disperse2"));
}

#[test]
fn eagle_exit_dispel_removes_the_configured_buff_type_family() {
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(90002, "Disperse2"),
        vec![8001],
        vec!["8001".into()],
    );

    let definition = registry::find(&behavior).unwrap();
    assert_eq!(definition.kind, BehaviorKind::DisperseTypeId);
    assert!(definition
        .supports
        .is_some_and(|supports| supports(&behavior)));
    let operations = remove_each_buff_family_ops(-1, &behavior).unwrap();
    let [RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(command)))] =
        operations.as_slice()
    else {
        panic!("expected one buff-family removal command");
    };
    assert_eq!(command.target_uid, -1);
    assert_eq!(command.selector, BuffRemoveSelector::IdOrType(8001));
    assert!(command.origin.key.matches(90002, "Disperse2"));

    for invalid in [Vec::new(), vec![0], vec![-1], vec![8001, 8002]] {
        assert!(!definition
            .supports
            .is_some_and(|supports| supports(&ParsedBehavior::new(90002, "Disperse2", invalid))));
    }
    let grouped = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(90002, "Disperse2"),
        vec![8001],
        vec!["8001,8002".into()],
    );
    assert!(!definition
        .supports
        .is_some_and(|supports| supports(&grouped)));
    assert!(registry::find_key(90002, "Disperse1").is_none());
}

#[test]
fn eagle_death_dispatches_the_exact_cleanup_through_buff_manager() {
    use crate::engine::{
        event::payload::{BattleEvent, EntityDiedEvent},
        manager::BattleManagers,
        runtime::{determinism::RoundDeterminism, drain},
        skill::{effect::SkillEffectCatalog, target::TargetContext},
    };

    crate::test_support::init_config();
    let mut fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                passive_skill: vec![30060141],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(1_000),
                buffs: vec![BuffInfo {
                    buff_id: Some(30061),
                    uid: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    fight.attacker.as_mut().unwrap().entitys[0].current_hp = Some(0);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);

    drain::run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::EntityDied(EntityDiedEvent {
            source_uid: -1,
            target_uid: 10,
        }),
    )
    .unwrap();

    assert!(!managers.buff.has_buff_id(-1, 30061));
}

#[test]
fn purify_x_keeps_the_limit_separate_from_status_arguments() {
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(20020, "PurifyX"),
        vec![1, 2, 4, 6],
        vec!["1".into(), "2".into(), "4".into(), "6".into()],
    );

    let Some(commands) = dispel_commands(11, &behavior) else {
        panic!("expected a limited purify command");
    };
    let [BuffCommand::Dispel(command)] = commands.as_slice() else {
        panic!("expected a limited purify command");
    };
    assert_eq!(command.target_uid, 11);
    assert_eq!(command.count, 1);
    assert_eq!(
        command.statuses,
        vec![
            BuffStatus::StatsDown,
            BuffStatus::Control,
            BuffStatus::NegativeStatus,
        ]
    );
    assert!(command.origin.key.matches(20020, "PurifyX"));
}

#[test]
fn enemy_purify_x_uses_its_exact_positive_status_row() {
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60064, "PurifyX"),
        vec![2, 1, 5],
        vec!["2".into(), "1".into(), "5".into()],
    );

    let commands = dispel_commands(-1, &behavior).unwrap();
    let [BuffCommand::Dispel(command)] = commands.as_slice() else {
        panic!("expected a limited enemy dispel command");
    };
    assert_eq!(command.count, 2);
    assert_eq!(
        command.statuses,
        vec![BuffStatus::StatsUp, BuffStatus::PositiveStatus]
    );
    assert!(command.origin.key.matches(60064, "PurifyX"));
}

#[test]
fn excluded_dispel_preserves_matching_buff_ids_and_types() {
    let behavior = ParsedBehavior::from_spec(
        crate::engine::skill::behavior::classify::BehaviorSpec::new(60060, "DisperseExclude"),
        vec![4150001, 2, 6],
        vec!["4150001".into(), "2,6".into()],
    );

    let BuffCommand::Dispel(command) = excluded_dispel_command(10, &behavior).unwrap() else {
        panic!("expected an excluded dispel command");
    };
    assert_eq!(
        command.statuses,
        vec![BuffStatus::StatsDown, BuffStatus::NegativeStatus]
    );
    assert_eq!(command.excluded_ids_or_types, vec![4150001]);
    assert!(command.origin.key.matches(60060, "DisperseExclude"));
}
