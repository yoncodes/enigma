use super::*;

fn pending_death(
    managers: &mut BattleManagers,
    target_uid: i64,
) -> crate::engine::manager::hp::PendingDeath {
    use crate::engine::manager::hp::{HpCommand, HpKill};
    managers
        .hp
        .execute_command(HpCommand::Kill(HpKill {
            origin: CommandOrigin {
                domain: RuleDomain::Lifecycle,
                key: DefinitionKey::new(0, "TestDeath"),
            },
            source_uid: 10,
            target_uid,
            config_effect: 1,
        }))
        .unwrap()
        .death
        .unwrap()
}

fn resolve_defeat(managers: &mut BattleManagers, fight: &Fight, target_uid: i64) -> DrainResult {
    let pool = TargetPool::from_fight_with_catalog(managers.catalog(), fight);
    let effects = SkillEffectCatalog::from_fight(config::configs::get(), fight);
    let mut determinism = RoundDeterminism::default();
    let death = pending_death(managers, target_uid);
    run_command_group(
        managers,
        &pool,
        &effects,
        &mut determinism,
        TargetContext::default(),
        [RuleOp::DeathSettlement(death)],
    )
    .unwrap()
}

#[test]
fn queued_death_updates_kill_task_for_an_entrant_before_dead() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let fight = Fight {
        episode_id: Some(493_708),
        fight_task_box: Some(
            crate::engine::manager::fight_task::configured_box(catalog, 493_708).unwrap(),
        ),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded_with_catalog(catalog, &fight).unwrap();
    managers.register_entity(&FightEntityInfo {
        uid: Some(-1),
        model_id: Some(116_370_081),
        current_hp: Some(1),
        team_type: Some(2),
        position: Some(1),
        ..Default::default()
    });
    let result = resolve_defeat(&mut managers, &fight, -1);

    assert!(
        matches!(
            result.frames[0].items.as_slice(),
            [
                FrameItem::Change(task),
                FrameItem::Change(death)
            ] if matches!(task.as_ref(), BattleChange::FightTask(change)
                if change.tasks[0].task_id == Some(370_803)
                    && change.tasks[0].values[0].progress == Some(1))
                && matches!(death.as_ref(), BattleChange::Death(transition)
                    if transition.target_uid == -1)
        ),
        "{:#?}",
        result.frames[0].items
    );
}

#[test]
fn enemy_death_awards_the_health_task_before_kill_progress() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let task = |task_id, max_progress| sonettobuf::FightTask {
        task_id: Some(task_id),
        status: Some(2),
        values: vec![sonettobuf::FightTaskValue {
            index: Some(1),
            progress: Some(0),
            max_progress: Some(max_progress),
            finished: Some(false),
        }],
    };
    let enemy = |uid, model_id, position, current_hp, max_hp| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(2),
        position: Some(position),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(max_hp),
            ..Default::default()
        }),
        ..Default::default()
    };

    for (boss_hp, nested_defeat, expected) in [
        (500, false, vec![381_106]),
        (499, false, vec![381_102, 381_106]),
        (750, true, vec![381_102, 381_106]),
    ] {
        let fight = Fight {
            defender: Some(FightTeam {
                entitys: vec![
                    enemy(-1, 120_384_121, 1, boss_hp, 1000),
                    enemy(-2, 120_384_112, 2, 100, 100),
                ],
                ..Default::default()
            }),
            fight_task_box: Some(sonettobuf::FightTaskBox {
                tasks: vec![task(381_102, 1), task(381_106, 6)],
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded_with_catalog(catalog, &fight).unwrap();
        let result = if nested_defeat {
            let primary_death = pending_death(&mut managers, -2);
            let reaction_death = pending_death(&mut managers, -1);
            let pool = TargetPool::from_fight_with_catalog(catalog, &fight);
            let effects = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
            let mut determinism = RoundDeterminism::default();
            let mut state = DrainState::new(TargetContext::default());
            let mut result = DrainResult::default();
            let root = push_root(
                &mut result.frames,
                FrameOwner::Command,
                FrameTrigger::Active,
            );
            drain_death_settlement_queue(
                &mut managers,
                &pool,
                &effects,
                &mut determinism,
                VecDeque::from([
                    queued_death_event_group_settlement(reaction_death, &root),
                    queued_death_event_group_settlement(primary_death, &root),
                ]),
                &mut result,
                &mut state,
                &[primary_death],
            )
            .unwrap();
            assert!(
                matches!(result.frames[0].items.first(), Some(FrameItem::Change(change))
                    if matches!(change.as_ref(), BattleChange::Death(death) if death.target_uid == -1))
            );
            result
        } else {
            resolve_defeat(&mut managers, &fight, -2)
        };
        let items = &result.frames[0].items[usize::from(nested_defeat)..];
        assert_eq!(items.len(), expected.len() + 1);
        assert!(matches!(items.last(), Some(FrameItem::Change(change))
                if matches!(change.as_ref(), BattleChange::Death(death) if death.target_uid == -2)));
        for (item, task_id) in items.iter().zip(expected) {
            assert!(matches!(item, FrameItem::Change(change)
                    if matches!(change.as_ref(), BattleChange::FightTask(change)
                       if change.tasks[0].task_id == Some(task_id)
                          && change.tasks[0].values[0].progress == Some(1))));
        }
    }
}
