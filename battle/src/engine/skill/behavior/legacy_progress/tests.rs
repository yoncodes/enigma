use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use crate::engine::{
    event::payload::{BattleEvent, EntityEntry},
    manager::{BattleManagers, fight_progress::LegacyProgressAdd},
    runtime::{
        determinism::RoundDeterminism,
        drain,
        executor::RuleOutcome,
        record::{FrameItem, FrameOwner, SemanticFrame},
    },
    skill::{
        action::{ActionEvent, SkillExecutionMode},
        effect::SkillEffectCatalog,
        rule::{CommandOrigin, DefinitionKey, RuleDomain, SetupStage},
        target::{TargetContext, TargetPool},
    },
};

#[test]
fn completed_actions_settle_progress_once_per_acting_holder() {
    let progress_skill = 117333503;
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        passive_skill: vec![progress_skill],
        current_hp: Some(100),
        ..Default::default()
    };
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(11)],
            ..Default::default()
        }),
        progress: Some(0),
        progress_max: Some(100),
        ..Default::default()
    };
    let catalog = SkillEffectCatalog::from_fight(crate::test_support::game_data(), &fight);
    assert!(catalog.issues(progress_skill).is_empty());
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);

    for (source_uid, skill_type, is_attack, mode, expected) in [
        (10, 1, true, SkillExecutionMode::Active, &[4][..]),
        (11, 2, false, SkillExecutionMode::Active, &[4][..]),
        (10, 2, true, SkillExecutionMode::Active, &[4, 4][..]),
        (10, 2, true, SkillExecutionMode::NestedExtra, &[][..]),
    ] {
        let result = drain::run_event(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                current_round: 1,
                ..Default::default()
            },
            BattleEvent::AllyAction(ActionEvent {
                source_uid,
                target_uid: 11,
                skill_id: 31200114,
                skill_type,
                is_attack,
                mode,
                ..Default::default()
            }),
        )
        .unwrap();
        let gains = result
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                RuleOutcome::LegacyProgressGain(change) => Some(change.delta),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(gains, expected);
        let mut owners = Vec::new();
        for frame in &result.frames {
            collect_progress_owners(frame, progress_skill, &mut owners);
        }
        if expected.is_empty() {
            assert!(owners.is_empty());
        } else {
            assert_eq!(owners, [source_uid]);
        }
    }
    managers.fight_progress.sync_fight(&mut fight);
    assert_eq!(fight.progress, Some(16));
}

fn collect_progress_owners(frame: &SemanticFrame, progress_skill: i32, owners: &mut Vec<i64>) {
    if let FrameOwner::Skill {
        source_uid,
        skill_id,
        ..
    } = frame.owner
        && skill_id == progress_skill
    {
        owners.push(source_uid);
    }
    for item in &frame.items {
        if let FrameItem::Child(child) = item {
            collect_progress_owners(child, progress_skill, owners);
        }
    }
}

#[test]
fn legacy_progress_rejects_surplus_action_trigger_arguments() {
    use super::{AddHandler, BehaviorConfigError, BehaviorHandler};

    let skill_id = 117333503;
    let db = crate::test_support::game_data();
    let catalog = SkillEffectCatalog::from_roots(db, [skill_id], []);
    let mut slot = catalog.get(skill_id).unwrap().slots[0].clone();
    slot.conditions[0].raw_args.push("7".to_owned());
    assert!(matches!(
        AddHandler::validate_slot(&slot, "502212#0#7"),
        Err(BehaviorConfigError::Unsupported)
    ));
}

#[test]
fn legacy_progress_initializes_only_at_opening() {
    let skill_id = 104318544;
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                passive_skill: vec![skill_id],
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let db = crate::test_support::game_data();
    let catalog = SkillEffectCatalog::from_fight(db, &fight);
    let initializer = &catalog.get(skill_id).unwrap().slots[0];
    assert!(initializer.compiled_subscriptions().unwrap().is_empty());
    let cold = SkillEffectCatalog::from_roots(db, [110500101], []);
    let initializer = &cold.get(110500101).unwrap().slots[0];
    assert_eq!(initializer.setup_keys(SetupStage::BattleStart, 0).len(), 1);
    assert!(initializer.compiled_subscriptions().unwrap().is_empty());
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let context = TargetContext::default();
    drain::run_setup_stage(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        SetupStage::EnterFight,
        0,
    )
    .unwrap();
    managers
        .fight_progress
        .add_legacy(LegacyProgressAdd {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60107, "ProgressChange"),
            },
            amount: 3,
        })
        .unwrap();
    drain::run_event(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        BattleEvent::EntityEntered {
            target_uid: 10,
            entry: EntityEntry::Summoned,
        },
    )
    .unwrap();
    managers.fight_progress.sync_fight(&mut fight);
    assert_eq!(fight.progress, Some(3));
    assert_eq!(fight.progress_max, Some(4));
}
