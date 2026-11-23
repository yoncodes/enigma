use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    skill::{
        action::{SkillActionEvent, SkillPhase},
        rule::DefinitionKey,
    },
};
use crate::test_support::init_config;

fn subscriber() -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 2292031,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::at_phase(
            EventKind::SkillAction,
            DefinitionKey::new(10006, "BeatBackDependOnAttackMe"),
            Some(SkillPhase::HitPassives),
        ),
        act_type: "BeatBackDependOnAttackMe".into(),
        effect_time: 401,
        effect_condition: 0,
        args: vec![312301611, 312301621],
        raw: "10006#312301611#312301621".into(),
    }
}

fn holder_subscriber() -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 21,
        buff_id: 222001721,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::at_phase(
            EventKind::SkillAction,
            DefinitionKey::new(302, "BeatBack"),
            Some(SkillPhase::HitPassives),
        ),
        act_type: "BeatBack".into(),
        effect_time: 401,
        effect_condition: 0,
        args: vec![0, 222001751],
        raw: "302#0#222001751".into(),
    }
}

fn attack(attacked_target_uids: Vec<i64>, damage_amount: i32) -> BattleEvent {
    BattleEvent::SkillAction(SkillActionEvent {
        source_uid: -1,
        card_index: 0,
        skill_id: 100,
        target_uid: attacked_target_uids.first().copied().unwrap_or_default(),
        target_uids: attacked_target_uids.clone(),
        attacked_target_uids,
        phase: SkillPhase::HitPassives,
        skill_slot: 1,
        is_attack: true,
        rank: 1,
        skill_type: 1,
        effect_tag: 1,
        assassinate: false,
        ignore_riposte: false,
        damage_amount,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode: crate::engine::skill::action::SkillExecutionMode::Active,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    })
}

#[test]
fn owner_and_ally_hits_select_distinct_configured_counter_skills() {
    init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let skill = |target_uids| match rule_ops(&pool, &subscriber(), &attack(target_uids, 10))
        .unwrap()
        .as_slice()
    {
        [RuleOp::Skill(invocation)] => invocation.clone(),
        output => panic!("expected one riposte skill, got {output:?}"),
    };

    let self_counter = skill(vec![10]);
    assert_eq!(self_counter.plan.skill_id, 312301611);
    assert_eq!(self_counter.target, SkillTarget::Explicit(-1));
    assert_eq!(self_counter.extra_skill_kind, Some(ExtraSkillKind::Riposte));
    assert_eq!(skill(vec![11]).plan.skill_id, 312301621);
    assert_eq!(skill(vec![11, 10]).plan.skill_id, 312301611);
}

#[test]
fn ignored_riposte_does_not_schedule_a_counter() {
    init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let mut event = attack(vec![10], 10);
    let BattleEvent::SkillAction(action) = &mut event else {
        unreachable!()
    };
    action.ignore_riposte = true;

    assert!(
        holder_rule_ops(&pool, &holder_subscriber(), &event)
            .unwrap()
            .is_empty()
    );
    assert!(rule_ops(&pool, &subscriber(), &event).unwrap().is_empty());
}

#[test]
fn shield_absorption_does_not_suppress_an_attack_triggered_riposte() {
    init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let shielded_hit = attack(vec![10], 0);

    assert!(matches!(
        rule_ops(&pool, &subscriber(), &shielded_hit)
            .unwrap()
            .as_slice(),
        [RuleOp::Skill(_)]
    ));
}

#[test]
fn holder_counter_only_answers_hits_on_its_owner() {
    init_config();
    let pool = TargetPool::from_fight(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });

    let own_hit = holder_rule_ops(&pool, &holder_subscriber(), &attack(vec![10], 10)).unwrap();
    let [RuleOp::Skill(invocation)] = own_hit.as_slice() else {
        panic!("expected one holder riposte, got {own_hit:?}");
    };
    assert_eq!(invocation.plan.skill_id, 222001751);
    assert_eq!(invocation.target, SkillTarget::Explicit(-1));
    assert_eq!(invocation.extra_skill_kind, Some(ExtraSkillKind::Riposte));
    assert!(
        holder_rule_ops(&pool, &holder_subscriber(), &attack(vec![11], 10))
            .unwrap()
            .is_empty()
    );

    let mut team_counter = holder_subscriber();
    team_counter.buff_id = 31210121;
    team_counter.args = vec![0, 31210132, 1, 31210121];
    team_counter.raw = "302#0#31210132#1#31210121".into();
    let ally_hit = holder_rule_ops(&pool, &team_counter, &attack(vec![11], 10)).unwrap();
    assert!(matches!(
        ally_hit.as_slice(),
        [
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                target_uid: 10,
                selector: BuffRemoveSelector::ExactId(31210121),
                ..
            }))),
            RuleOp::Skill(invocation),
        ] if invocation.plan.skill_id == 31210132
            && invocation.target == SkillTarget::Explicit(-1)
            && invocation.extra_skill_kind == Some(ExtraSkillKind::Riposte)
    ));
}

#[test]
fn riposte_registry_owns_phase_actor_and_argument_shape() {
    let dependent = super::super::registry::find(10006, "BeatBackDependOnAttackMe").unwrap();
    let holder = super::super::registry::find(302, "BeatBack").unwrap();

    for definition in [dependent, holder] {
        assert_eq!(
            definition.runtime.event_override,
            Some(EventKind::SkillAction)
        );
        assert_eq!(
            definition.runtime.phase_override,
            Some(SkillPhase::HitPassives)
        );
        assert_eq!(
            definition.runtime.actor_scope,
            super::super::registry::RuntimeActorScope::OpposingTeam
        );
    }
    assert!(supports_dependent(&[312301611, 312301621]));
    assert!(!supports_dependent(&[312301611]));
    assert!(!supports_dependent(&[312301611, 0]));
    assert!(supports_holder(&[0, 31210132, 1, 31210121]));
    assert!(!supports_holder(&[0, 31210132, 1]));
    assert!(!supports_holder(&[0, 31210132, 1, 0]));
}
