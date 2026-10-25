use super::*;

#[test]
fn ally_action_settles_the_acting_owners_take_stage_buff() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        buff_id: Some(30940191),
                        uid: Some(2),
                        from_uid: Some(10),
                        duration: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(20),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        buff_id: Some(30940191),
                        uid: Some(3),
                        from_uid: Some(20),
                        duration: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: Vec::new(),
    });

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        [RuleOp::Skill(SkillInvocation {
            mode: crate::engine::skill::action::SkillExecutionMode::Active,
            ..SkillRequest {
                source_uid: 10,
                skill_id: 100,
            }
            .into()
        })],
    )
    .unwrap();

    assert!(managers.buff.snapshot(10, 2).is_none());
    assert!(managers.buff.snapshot(20, 3).is_some());
    assert!(result.outcomes.iter().any(|outcome| matches!(
        outcome,
        RuleOutcome::Buff(changes)
            if changes.origin.key.opcode == 212
                && changes.change.removed.iter().any(|removed| removed.buff.uid == Some(2))
    )));
}

#[test]
fn queued_reactions_preserve_dispatch_pass_order() {
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::subscriber::SkillSubscriber,
    };

    let reaction = |owner_uid, skill_id, key| {
        (
            SkillSubscriber {
                owner_uid,
                skill_id,
                slot_index: Some(0),
                key: SubscriptionKey::new(EventKind::SkillEffectStarted, key),
            },
            RuleOp::Skill(
                SkillRequest {
                    source_uid: owner_uid,
                    skill_id,
                }
                .into(),
            ),
        )
    };
    let started = BattleEvent::SkillEffectStarted(
        crate::engine::skill::action::SkillActionEvent {
            source_uid: 10,
            skill_id: 31050151,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: Vec::new(),
            phase: crate::engine::skill::action::SkillPhase::Immediate,
            skill_slot: 0,
            is_attack: true,
            rank: 1,
            skill_type: 1,
            effect_tag: 1,
            assassinate: false,
            ignore_riposte: false,
            damage_amount: 0,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 1,
            mode: crate::engine::skill::action::SkillExecutionMode::Active,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        }
        .into(),
    );
    let queued = queued_reactions(
        &TargetPool::default(),
        dispatcher::DispatchBatch {
            skills: vec![reaction(
                -1,
                109380003,
                DefinitionKey::new(502202, "ActiveUseSkill"),
            )],
            following: vec![dispatcher::DispatchBatch {
                skills: vec![reaction(
                    10,
                    433911,
                    DefinitionKey::new(403203, "SkillExtraType"),
                )],
                ..Default::default()
            }],
            ..Default::default()
        },
        &started,
        Some(&[0]),
        None,
        None,
        None,
        None,
        None,
    )
    .unwrap();

    assert_eq!(
        queued
            .iter()
            .map(|reaction| reaction
                .subscriber
                .as_ref()
                .map(|subscriber| subscriber.owner_uid))
            .collect::<Vec<_>>(),
        vec![Some(-1), Some(10)]
    );
}
