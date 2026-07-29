use crate::engine::{
    event::payload::BattleEvent,
    skill::{
        action::{SkillInvocation, SkillRequest, SkillTarget},
        buff_act::registry::BuffActKind,
        condition::extra::ExtraSkillKind,
        rule::output::RuleOp,
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub fn shielded_ally_rule_ops(
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::BeatBackByCounter) {
        return None;
    }
    let BattleEvent::Hit(hit) = event else {
        return Some(Vec::new());
    };
    if !subscriber.owner_alive
        || pool.source_is_attacker(hit.source_uid) == pool.source_is_attacker(subscriber.owner_uid)
        || pool.source_is_attacker(hit.target_uid) != pool.source_is_attacker(subscriber.owner_uid)
        || hit.shield_absorbed <= 0
    {
        return Some(Vec::new());
    }
    let [skill_id] = subscriber.args.as_slice() else {
        return None;
    };
    if *skill_id <= 0 {
        return None;
    }
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: subscriber.owner_uid,
        skill_id: *skill_id,
    }
    .into();
    invocation.target = SkillTarget::Explicit(hit.source_uid);
    invocation.extra_skill_kind = Some(ExtraSkillKind::Riposte);
    Some(vec![RuleOp::Skill(invocation)])
}

pub fn rule_ops(
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::BeatBackDependOnAttackMe) {
        return None;
    }
    let BattleEvent::SkillAction(action) = event else {
        return Some(Vec::new());
    };
    let owner_team = pool.source_is_attacker(subscriber.owner_uid);
    if !subscriber.owner_alive
        || !action.is_attack
        || pool.source_is_attacker(action.source_uid) == owner_team
    {
        return Some(Vec::new());
    }
    let owner_was_attacked = action.attacked_target_uids.contains(&subscriber.owner_uid);
    let ally_was_attacked = action.attacked_target_uids.iter().any(|target_uid| {
        *target_uid != subscriber.owner_uid && pool.source_is_attacker(*target_uid) == owner_team
    });
    let skill_id = if owner_was_attacked {
        subscriber.args.first()
    } else if ally_was_attacked {
        subscriber.args.get(1)
    } else {
        return Some(Vec::new());
    }
    .copied()
    .filter(|skill_id| *skill_id > 0)?;
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: subscriber.owner_uid,
        skill_id,
    }
    .into();
    invocation.target = SkillTarget::Explicit(action.source_uid);
    invocation.extra_skill_kind = Some(ExtraSkillKind::Riposte);
    Some(vec![RuleOp::Skill(invocation)])
}

pub fn holder_rule_ops(
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::BeatBack) {
        return None;
    }
    let BattleEvent::SkillAction(action) = event else {
        return Some(Vec::new());
    };
    let owner_team = pool.source_is_attacker(subscriber.owner_uid);
    if !subscriber.owner_alive
        || !action.is_attack
        || !action.attacked_target_uids.contains(&subscriber.owner_uid)
        || pool.source_is_attacker(action.source_uid) == owner_team
    {
        return Some(Vec::new());
    }
    let skill_id = holder_skill(&subscriber.args)?;
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: subscriber.owner_uid,
        skill_id,
    }
    .into();
    invocation.target = SkillTarget::Explicit(action.source_uid);
    invocation.extra_skill_kind = Some(ExtraSkillKind::Riposte);
    Some(vec![RuleOp::Skill(invocation)])
}

pub fn holder_skill(args: &[i32]) -> Option<i32> {
    match args {
        [0, skill_id] if *skill_id > 0 => Some(*skill_id),
        _ => None,
    }
}

pub fn counter_skill(args: &[i32]) -> Option<i32> {
    match args {
        [skill_id] if *skill_id > 0 => Some(*skill_id),
        _ => None,
    }
}

pub fn supports_holder(args: &[i32]) -> bool {
    holder_skill(args).is_some()
}

pub fn supports_dependent(args: &[i32]) -> bool {
    matches!(args, [self_skill, ally_skill] if *self_skill > 0 && *ally_skill > 0)
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        event::{kind::EventKind, payload::HitEvent, subscription::SubscriptionKey},
        manager::hp::HurtDamageFromType,
        skill::{
            action::{SkillActionEvent, SkillPhase},
            rule::{CommandOrigin, DefinitionKey, RuleDomain},
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

    fn shield_counter_subscriber() -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 22,
            buff_id: 30940182,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::Riposte,
                DefinitionKey::new(802, "BeatBackByCounter"),
            ),
            act_type: "BeatBackByCounter".into(),
            effect_time: 401,
            effect_condition: 0,
            args: vec![30940172],
            raw: "802#30940172".into(),
        }
    }

    fn hit(source_uid: i64, target_uid: i64) -> BattleEvent {
        BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "SkillDamage"),
            },
            source_uid,
            target_uid,
            skill_id: 1,
            amount: 10,
            shield_absorbed: 0,
            damage_from: HurtDamageFromType::Skill,
            assassinate: false,
        })
    }

    fn attack(attacked_target_uids: Vec<i64>, damage_amount: i32) -> BattleEvent {
        BattleEvent::SkillAction(SkillActionEvent {
            source_uid: -1,
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
            damage_amount,
            kill_count: 0,
            crit_count: 0,
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
    }

    #[test]
    fn shield_counter_uses_the_configured_skill_when_the_hit_absorbed_a_shield() {
        init_config();
        let fight = Fight {
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
        };
        let pool = TargetPool::from_fight(&fight);
        let mut shielded_hit = hit(-1, 11);
        let BattleEvent::Hit(shielded) = &mut shielded_hit else {
            unreachable!()
        };
        shielded.shield_absorbed = 20;

        let output =
            shielded_ally_rule_ops(&pool, &shield_counter_subscriber(), &shielded_hit).unwrap();
        let [RuleOp::Skill(invocation)] = output.as_slice() else {
            panic!("expected one shield counter, got {output:?}");
        };
        assert_eq!(invocation.plan.skill_id, 30940172);
        assert_eq!(invocation.target, SkillTarget::Explicit(-1));
        assert_eq!(invocation.extra_skill_kind, Some(ExtraSkillKind::Riposte));

        assert!(
            shielded_ally_rule_ops(&pool, &shield_counter_subscriber(), &hit(-1, 11))
                .unwrap()
                .is_empty()
        );
        let mut allied_hit = hit(10, 11);
        let BattleEvent::Hit(allied) = &mut allied_hit else {
            unreachable!()
        };
        allied.shield_absorbed = 20;
        assert!(
            shielded_ally_rule_ops(&pool, &shield_counter_subscriber(), &allied_hit)
                .unwrap()
                .is_empty()
        );
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
    }
}
