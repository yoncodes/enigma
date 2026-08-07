use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector},
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        action::{SkillExecutionMode, SkillInvocation, SkillRequest},
        condition::extra::skill_kind_from_is_extra,
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

use super::registry::BuffActKind;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [1, rate, lock_buff_id, owner_skill_id]
        if *rate > 0 && *lock_buff_id > 0 && *owner_skill_id > 0)
}

pub fn referenced_buff(args: &[i32]) -> Option<i32> {
    supports(args).then_some(args[2])
}

pub fn referenced_skill(args: &[i32]) -> Option<i32> {
    supports(args).then_some(args[3])
}

pub fn grant_transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    super::changed_features(event, BuffActKind::ContractCastChannel)
        .into_iter()
        .filter_map(|(feature, _)| grant_rule_op(managers, &feature).map(|op| (feature, op)))
        .collect()
}

fn grant_rule_op(managers: &BattleManagers, feature: &ActiveBuffFeature) -> Option<RuleOp> {
    let args = feature.values.get(1..)?;
    if !supports(args) {
        return None;
    }
    let [1, _, lock_buff_id, _] = args else {
        return None;
    };
    let bound_uid = managers.contract.bound_uid(feature.owner_uid)?;
    if managers.hp.current(bound_uid) <= 0 {
        return None;
    }
    Some(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
        BuffGrant {
            origin: super::feature_command_origin(feature)?,
            source_uid: feature.owner_uid,
            target_uid: bound_uid,
            buff_id: *lock_buff_id,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        },
    ))))
}

pub fn rule_ops(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::ContractCastChannel)
        || event.kind() != EventKind::RoundStart
        || !supports(&subscriber.args)
    {
        return None;
    }
    let [1, rate, lock_buff_id, owner_skill_id] = subscriber.args.as_slice() else {
        return None;
    };
    let Some(bound_uid) = managers.contract.bound_uid(subscriber.owner_uid) else {
        return Some(Vec::new());
    };
    if managers.hp.current(subscriber.owner_uid) <= 0 || managers.hp.current(bound_uid) <= 0 {
        return Some(Vec::new());
    }
    let Some(bound_skill_id) = managers
        .entity
        .snapshot(bound_uid)
        .and_then(|entity| entity.ex_skill)
        .filter(|skill_id| *skill_id > 0)
    else {
        return Some(Vec::new());
    };
    let Some(lock_buff_uid) = managers.buff.buff_id_uid(bound_uid, *lock_buff_id) else {
        return Some(Vec::new());
    };
    let origin = super::command_origin(subscriber)?;
    let current_hp = managers.hp.current(bound_uid).max(0);
    let loss =
        (i64::from(current_hp) * i64::from(*rate) / 1000).clamp(0, i64::from(i32::MAX)) as i32;
    let mut ops = Vec::with_capacity(4);
    if loss > 0 {
        ops.push(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
            HpLoss {
                origin,
                source_uid: subscriber.owner_uid,
                target_uid: bound_uid,
                amount: loss,
                config_effect: 0,
                hurt: Some(HurtInfoData {
                    from_uid: subscriber.owner_uid,
                    is_crit: false,
                    career_restraint: false,
                    reduce_hp: 0,
                    effect_id: 0,
                    skill_id: 0,
                    damage_from: HurtDamageFromType::Buff,
                    buff_act_id: subscriber.key.definition.opcode,
                    buff_uid: subscriber.buff_uid,
                    hurt_effect_type: 0,
                    display_amount: None,
                }),
            },
        ))));
    }
    ops.push(RuleOp::Skill(invocation(
        bound_uid,
        bound_skill_id,
        catalog,
    )));
    ops.push(RuleOp::Skill(invocation(
        subscriber.owner_uid,
        *owner_skill_id,
        catalog,
    )));
    ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
        BuffRemove {
            origin,
            target_uid: bound_uid,
            selector: BuffRemoveSelector::Uid(lock_buff_uid),
        },
    ))));
    Some(ops)
}

fn invocation(owner_uid: i64, skill_id: i32, catalog: &SkillEffectCatalog) -> SkillInvocation {
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: owner_uid,
        skill_id,
    }
    .into();
    invocation.mode = SkillExecutionMode::Active;
    invocation.extra_skill_kind = skill_kind_from_is_extra(catalog.extra_kind(skill_id));
    invocation
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        event::{payload::BuffChangeEvent, subscription::SubscriptionKey},
        manager::{
            buff::CommandOrigin,
            contract::{ContractCommand, ContractManager},
        },
        skill::rule::{DefinitionKey, RuleDomain},
    };

    fn origin() -> CommandOrigin {
        CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: DefinitionKey::new(836, "ContractCastChannel"),
        }
    }

    fn managers(with_lock: bool) -> BattleManagers {
        crate::test_support::init_config();
        let mut managers = BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(-1),
                        team_type: Some(1),
                        current_hp: Some(2_000),
                        attr: Some(HeroAttribute {
                            hp: Some(2_000),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(22),
                        team_type: Some(1),
                        ex_skill: Some(307_001_333),
                        current_hp: Some(1_000),
                        attr: Some(HeroAttribute {
                            hp: Some(1_000),
                            ..Default::default()
                        }),
                        buffs: with_lock
                            .then(|| BuffInfo {
                                uid: Some(77),
                                buff_id: Some(31_000_151),
                                from_uid: Some(-1),
                                duration: Some(1),
                                ..Default::default()
                            })
                            .into_iter()
                            .collect(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        });
        managers.contract = ContractManager::default();
        managers
            .contract
            .execute(ContractCommand::Offer {
                origin: origin(),
                owner_uid: -1,
                candidates: vec![22],
            })
            .unwrap();
        managers
            .contract
            .execute(ContractCommand::SelectOwner {
                owner_uid: -1,
                bound_uid: 22,
            })
            .unwrap();
        managers
            .contract
            .execute(ContractCommand::SelectBound {
                owner_uid: -1,
                bound_uid: 22,
            })
            .unwrap();
        managers
    }

    #[test]
    fn carrier_grant_locks_the_selected_bound_ally() {
        let ops = grant_transaction_rule_ops(
            &managers(false),
            &BattleEvent::BuffAdded(BuffChangeEvent {
                source_uid: -1,
                target_uid: -1,
                buff_uid: 1364,
                buff_id: 31_000_431,
                before_amount: 0,
                after_amount: 1,
                act_id: 0,
                act_value: 0,
            }),
        );

        assert!(matches!(
            ops.as_slice(),
            [(
                _,
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                    source_uid: -1,
                    target_uid: 22,
                    buff_id: 31_000_151,
                    ..
                })))
            )]
        ));
    }

    #[test]
    fn carrier_grant_rejects_unsupported_arguments_before_mutation() {
        let feature = |values: Vec<i32>| ActiveBuffFeature {
            owner_uid: -1,
            source_uid: -1,
            buff_uid: 1364,
            buff_id: 31_000_431,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "ContractCastChannel".to_owned(),
            effect_time: 104,
            effect_condition: 0,
            raw: values
                .iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join("#"),
            values,
        };
        let managers = managers(false);

        assert!(
            grant_rule_op(
                &managers,
                &feature(vec![836, 1, -150, 31_000_151, 31_000_441])
            )
            .is_none()
        );
        assert!(grant_rule_op(&managers, &feature(vec![836, 1, 150, 0, 31_000_441])).is_none());
        assert!(grant_rule_op(&managers, &feature(vec![836, 1, 150, 31_000_151, 0])).is_none());
    }

    #[test]
    fn round_start_loses_hp_casts_both_ultimates_then_removes_the_lock() {
        let managers = managers(true);
        let subscriber = BuffActSubscriber {
            owner_uid: -1,
            source_uid: -1,
            buff_uid: 1364,
            buff_id: 31_000_431,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundStart,
                DefinitionKey::new(836, "ContractCastChannel"),
            ),
            act_type: "ContractCastChannel".to_owned(),
            effect_time: 104,
            effect_condition: 0,
            args: vec![1, 150, 31_000_151, 31_000_441],
            raw: "836#1#150#31000151#31000441".to_owned(),
        };

        let ops = rule_ops(
            &managers,
            &SkillEffectCatalog::from_game_db(config::configs::get()),
            &subscriber,
            &BattleEvent::RoundStart,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                    source_uid: -1,
                    target_uid: 22,
                    amount: 150,
                    hurt: Some(HurtInfoData {
                        damage_from: HurtDamageFromType::Buff,
                        buff_act_id: 836,
                        buff_uid: 1364,
                        ..
                    }),
                    ..
                }))),
                RuleOp::Skill(SkillInvocation {
                    plan: SkillRequest {
                        source_uid: 22,
                        skill_id: 307_001_333,
                    },
                    mode: SkillExecutionMode::Active,
                    ..
                }),
                RuleOp::Skill(SkillInvocation {
                    plan: SkillRequest {
                        source_uid: -1,
                        skill_id: 31_000_441,
                    },
                    mode: SkillExecutionMode::Active,
                    ..
                }),
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                    target_uid: 22,
                    selector: BuffRemoveSelector::Uid(77),
                    ..
                })))
            ]
        ));
    }
}
