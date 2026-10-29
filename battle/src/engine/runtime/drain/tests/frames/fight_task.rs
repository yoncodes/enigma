use super::*;

#[test]
fn queued_death_updates_kill_task_before_dead() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let fight = Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(116_370_081),
                current_hp: Some(0),
                ..Default::default()
            }],
            ..Default::default()
        }),
        fight_task_box: Some(
            crate::engine::manager::fight_task::configured_box(catalog, 493_708).unwrap(),
        ),
        ..Default::default()
    };
    let pool = TargetPool::from_fight_with_catalog(catalog, &fight);
    let mut managers = BattleManagers::seeded_with_catalog(catalog, &fight);
    let effects = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut determinism = RoundDeterminism::default();

    let result = run_command_group(
        &mut managers,
        &pool,
        &effects,
        &mut determinism,
        TargetContext::default(),
        [RuleOp::DeathSettlement(
            crate::engine::manager::hp::DeathTransition {
                source_uid: 10,
                target_uid: -1,
            },
        )],
    )
    .unwrap();

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
