use super::*;
use crate::engine::{
    event::kind::EventKind,
    manager::{
        BattleManagers,
        card::{CARD_PLAY_ORIGIN, CardChangeKind, CardInsertSovereign, CardManager},
    },
    packet::timeline,
    runtime::{
        determinism::RoundDeterminism,
        drain,
        executor::RuleOutcome,
        record::{FrameItem, FrameOwner},
    },
    skill::{
        behavior,
        condition::{self, ParsedConditionKind, registry::ReactionFrameScope},
        effect::SkillEffectCatalog,
        rule::DefinitionKey,
        target::{TargetContext, TargetPool},
    },
};
use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn initial_boost_fires_once_for_owner() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: [10, 11]
                .into_iter()
                .map(|uid| FightEntityInfo {
                    uid: Some(uid),
                    model_id: Some(3147),
                    team_type: Some(1),
                    current_hp: Some(1000),
                    passive_skill: if uid == 10 {
                        vec![31471154]
                    } else {
                        Vec::new()
                    },
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let db = config::configs::get();
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(db, &fight);
    let slot = &catalog.get(31471154).unwrap().slots[0];
    assert_eq!((slot.limit, slot.round_limit), (1, 0));
    assert_eq!(slot.behavior.raw_args, ["8"]);
    assert!(behavior::is_slot_supported(slot));
    assert!(behavior::registry::find_key(60296, "None").is_none());
    let definition = condition::registry::find_key(10500, "None").unwrap();
    assert_eq!(
        definition.reaction_frame_scope,
        ReactionFrameScope::Independent
    );
    assert!(condition::registry::find_key(10500, "UnnamedMoveCard").is_none());
    let condition = &slot.conditions[0];
    assert_eq!(condition.kind, ParsedConditionKind::SovereignCardCreated);
    assert_eq!(
        condition::satisfied_condition(condition, DefinitionKey::new(10500, "None")),
        *condition
    );
    assert!(condition::registry::parse(10500, "None", &["8".to_owned()]).is_none());
    assert!(
        slot.compiled_subscriptions()
            .unwrap()
            .iter()
            .any(|key| key.event == EventKind::SovereignCardCreated)
    );
    let mut unsupported = slot.behavior.clone();
    unsupported.args = vec![2];
    unsupported.raw_args = vec!["2".to_owned()];
    assert!(!behavior::is_supported(&unsupported));
    unsupported.args = vec![8];
    unsupported.raw_args = vec!["8,2".to_owned()];
    assert!(!behavior::is_supported(&unsupported));
    unsupported.raw_args.clear();
    assert!(!behavior::is_supported(&unsupported));

    let normal = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.card = CardManager::new(vec![normal.clone(); 4]);
    let mut determinism = RoundDeterminism::default();
    let insert = |owner_uid| {
        RuleOp::Command(BattleCommand::Card(CardCommand::InsertSovereign(
            CardInsertSovereign {
                origin: CARD_PLAY_ORIGIN,
                target_uid: owner_uid,
                skill_id: 31472131,
                hero_id: 3147,
                max_position: 9,
                team_type: 1,
            },
        )))
    };
    let unrelated = drain::run_command_group(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        [insert(11)],
    )
    .unwrap();
    assert!(!unrelated.outcomes.iter().any(|outcome| matches!(outcome,
        RuleOutcome::Card(changes) if changes.kind == CardChangeKind::InitialSovereignBoost)));
    managers
        .execute_card(CardCommand::Dissolve {
            origin: CARD_PLAY_ORIGIN,
            card_index: 4,
        })
        .unwrap();
    let first = drain::run_command_group(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        [insert(10)],
    )
    .unwrap();
    assert_eq!(
        first
            .outcomes
            .iter()
            .filter(|outcome| matches!(outcome,
        RuleOutcome::Card(changes) if changes.kind == CardChangeKind::InitialSovereignBoost))
            .count(),
        1
    );
    let FrameItem::Child(event) = &first.frames[1].items[0] else {
        panic!("independent event group");
    };
    let FrameItem::Child(skill) = &event.items[0] else {
        panic!("passive skill owner");
    };
    assert!(matches!(
        skill.owner,
        FrameOwner::Skill {
            source_uid: 10,
            skill_id: 31471154,
            target_uid: Some(10),
            ..
        }
    ));
    let FrameItem::Child(transaction) = &skill.items[0] else {
        panic!("initial boost lifecycle transaction");
    };
    assert_eq!(transaction.owner, FrameOwner::InitialSovereignBoost);
    let projected = timeline::project_for_version(&first.frames, 7).unwrap();
    let skill_step = projected[1].act_effect[0]
        .fight_step
        .as_ref()
        .unwrap()
        .act_effect[0]
        .fight_step
        .as_ref()
        .unwrap();
    assert_eq!(skill_step.act_id, Some(31471154));
    let transaction = skill_step.act_effect[0].fight_step.as_ref().unwrap();
    assert!(!transaction.fake_timeline.unwrap_or_default());
    assert_eq!(
        managers.card.hand()[0].card_dataes[0].value.as_deref(),
        Some(r#"{"lock":true,"strengthen":{"1":4,"2":4,"3":4,"4":4}}"#)
    );
    assert_eq!(&managers.card.hand()[1..], vec![normal.clone(); 4]);
    managers
        .execute_card(CardCommand::Dissolve {
            origin: CARD_PLAY_ORIGIN,
            card_index: 0,
        })
        .unwrap();
    let second = drain::run_command_group(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        [insert(10)],
    )
    .unwrap();
    assert!(!second.outcomes.iter().any(|outcome| matches!(outcome,
        RuleOutcome::Card(changes) if changes.kind == CardChangeKind::InitialSovereignBoost)));
    assert_eq!(
        managers.card.hand()[4].card_dataes[0].value.as_deref(),
        Some(r#"{"lock":true,"strengthen":{"1":0,"2":0,"3":0,"4":0}}"#)
    );
    assert_eq!(&managers.card.hand()[..4], vec![normal; 4]);
}
