use crate::engine::{
    manager::buff::{
        BuffAmount, BuffChildUidReservation, BuffCommand, BuffGrant, BuffGrantChild, BuffRemove,
        BuffRemoveSelector, BuffSetAmount,
    },
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub(super) struct Handler;

pub(super) fn supports_conversion(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [limit, output_buff_id] if *limit > 0 && *output_buff_id > 0)
}

pub(super) fn supports_poison_settlement_without_duration_cost(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [1, 1, rate] if *rate > 0)
}
pub(super) fn supports_poison_settlement_with_duration_cost(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [1, 0, rate] if *rate > 0)
}

impl BehaviorHandler for Handler {
    fn emit_ops(
        mut context: BehaviorOpContext<'_>,
        behavior: &ParsedBehavior,
    ) -> Option<Vec<RuleOp>> {
        match behavior.spec.kind {
            BehaviorKind::CatapultBuff => catapult_ops(&mut context, behavior),
            BehaviorKind::PoisonConvertToTargetBuff
            | BehaviorKind::PoisonConvertToPowerfulPoisonBuff => {
                convert_poison_ops(&context, behavior)
            }
            BehaviorKind::ConsumePoisonSettleDeadlyPoison => consume_poison_ops(&context, behavior),
            BehaviorKind::SettlePoisonWithoutDurationCost => settle_poison_ops(context, behavior),
            _ => None,
        }
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            skills: Vec::new(),
            buffs: match behavior.spec.kind {
                BehaviorKind::CatapultBuff => behavior.arg(3),
                BehaviorKind::PoisonConvertToTargetBuff
                | BehaviorKind::PoisonConvertToPowerfulPoisonBuff => behavior.arg(1),
                _ => None,
            }
            .into_iter()
            .collect(),
            models: Vec::new(),
            summons: Vec::new(),
        }
    }
}

fn settle_poison_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
    let [1, mode, rate] = behavior.args.as_slice() else { return None; };
    if !matches!(*mode, 0 | 1) { return None; }
    let origin = super::command_origin(behavior)?;
    let features = context.managers.buff.active_features_for_owner(&context.managers.hp, context.target_uid);
    let mut ops = Vec::new();
    let advance_duration = *mode == 0 && !context.managers.buff.has_buff_act_kind(
        context.target_uid, crate::engine::skill::buff_act::registry::BuffActKind::LockPoison,
    );
    for feature in features.into_iter().filter(|feature| {
        crate::engine::skill::buff_act::is_kind(feature, crate::engine::skill::buff_act::registry::BuffActKind::Poison)
            || crate::engine::skill::buff_act::is_kind(feature, crate::engine::skill::buff_act::registry::BuffActKind::PowerfulPoison)
    }) {
        let Some(subscriber) = crate::engine::skill::buff_act::subscriber_from_feature(
            feature, crate::engine::event::kind::EventKind::RoundEnd,
        ) else { continue; };
        let damage = if crate::engine::skill::buff_act::subscriber_is_kind(&subscriber,
            crate::engine::skill::buff_act::registry::BuffActKind::PowerfulPoison) {
            crate::engine::skill::buff_act::powerful_poison::settlement_ops(
                context.managers, context.pool, context.determinism, &subscriber, false,
            ).and_then(|ops| ops.into_iter().next())
        } else {
            crate::engine::skill::buff_act::damage_over_time::detonation_rule_op(
                context.managers, context.pool, context.determinism, &subscriber, false,
            )
        };
        let Some(RuleOp::Command(BattleCommand::Hp(crate::engine::manager::hp::HpCommand::Lose(mut loss)))) = damage else { continue; };
        loss.amount = loss.amount.saturating_mul(*rate) / 1000;
        loss.origin = origin;
        loss.config_effect = behavior.spec.key.opcode;
        if let Some(hurt) = &mut loss.hurt { hurt.display_amount = Some(loss.amount); }
        if loss.amount > 0 { ops.push(RuleOp::Command(BattleCommand::Hp(crate::engine::manager::hp::HpCommand::Lose(loss)))); }
        if advance_duration {
            let duration = context.managers.buff.snapshot(context.target_uid, subscriber.buff_uid)
                .and_then(|buff| buff.duration).unwrap_or(0);
            if duration > 0 {
                let advance = crate::engine::manager::buff::BuffDurationAdvance::new(
                    crate::engine::skill::buff_act::effect_time::ROUND_END_ENTITY_SETTLEMENT,
                    vec![context.target_uid], Some(vec![subscriber.buff_uid]),
                )?;
                ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::AdvanceDuration(advance))));
            }
        }
    }
    Some(ops)
}

fn convert_poison_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [limit, output_buff_id] = behavior.args.as_slice() else {
        return None;
    };
    if *limit <= 0 || *output_buff_id <= 0 {
        return None;
    }
    let mut poison = context
        .managers
        .buff
        .active_features_for_owner(&context.managers.hp, context.target_uid)
        .into_iter()
        .filter(|feature| {
            crate::engine::skill::buff_act::is_kind(
                feature,
                crate::engine::skill::buff_act::registry::BuffActKind::Poison,
            )
        })
        .collect::<Vec<_>>();
    poison.sort_by_key(|feature| feature.buff_uid);
    poison.dedup_by_key(|feature| feature.buff_uid);

    let origin = super::command_origin(behavior)?;
    let mut remaining = *limit;
    let mut ops = Vec::new();
    for feature in poison {
        let consumed = remaining.min(feature.amount.max(1));
        ops.push(consume_poison_op(
            origin,
            context.target_uid,
            feature.buff_uid,
            feature.amount,
            consumed,
        ));
        remaining -= consumed;
        if remaining == 0 {
            break;
        }
    }
    let converted = *limit - remaining;
    if converted > 0 {
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin,
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                buff_id: *output_buff_id,
                amount: Some(converted),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))));
    }
    Some(ops)
}

fn consume_poison_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [limit] = behavior.args.as_slice() else {
        return None;
    };
    if *limit <= 0 {
        return None;
    }
    let features = context.managers.buff.active_features_for_owner(&context.managers.hp, context.target_uid);
    let deadly = features.iter().find(|feature| {
        crate::engine::skill::buff_act::is_kind(
                feature,
                crate::engine::skill::buff_act::registry::BuffActKind::DeadlyPoison,
            )
    })?;
    let mut poison = features
        .iter()
        .filter(|feature| {
            feature.owner_uid == context.target_uid
                && crate::engine::skill::buff_act::is_kind(
                    feature,
                    crate::engine::skill::buff_act::registry::BuffActKind::Poison,
                )
        })
        .collect::<Vec<_>>();
    poison.sort_by_key(|feature| feature.buff_uid);

    let origin = super::command_origin(behavior)?;
    let mut remaining = *limit;
    let mut ops = Vec::new();
    for feature in poison {
        let consumed = remaining.min(feature.amount.max(1));
        ops.push(consume_poison_op(
            origin,
            context.target_uid,
            feature.buff_uid,
            feature.amount,
            consumed,
        ));
        remaining -= consumed;
        if remaining == 0 {
            break;
        }
    }
    let consumed = *limit - remaining;
    if consumed == 0 {
        return Some(Vec::new());
    }
    ops.extend(
        crate::engine::skill::buff_act::deadly_poison::settlement_ops(
            context.managers,
            deadly,
            origin,
            consumed,
        )?,
    );
    Some(ops)
}

fn consume_poison_op(
    origin: crate::engine::manager::buff::CommandOrigin,
    target_uid: i64,
    buff_uid: i64,
    available: i32,
    consumed: i32,
) -> RuleOp {
    if consumed >= available.max(1) {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
            origin,
            target_uid,
            selector: BuffRemoveSelector::Uid(buff_uid),
        })))
    } else {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetAmount(BuffSetAmount {
            origin,
            target_uid,
            buff_uid,
            amount: BuffAmount::Layer(available - consumed),
        })))
    }
}

fn catapult_ops(
    context: &mut BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [
        reserved_children,
        duration,
        initial_instances,
        buff_id,
        hop_instances,
        hop_count,
    ] = behavior.args.as_slice()
    else {
        return None;
    };
    if *reserved_children <= 0
        || *duration <= 0
        || *initial_instances <= 0
        || *buff_id <= 0
        || *hop_instances <= 0
        || *hop_count < 0
    {
        return None;
    }
    let origin = super::command_origin(behavior)?;
    let mut current = context.target_uid;
    let mut deliveries = vec![(current, *initial_instances)];
    for _ in 0..*hop_count {
        let candidates = context
            .pool
            .enemies(context.source_uid, false)
            .iter()
            .filter(|entity| entity.uid != current && context.managers.hp.current(entity.uid) > 0)
            .map(|entity| entity.uid)
            .collect::<Vec<_>>();
        let Some(index) = context.determinism.lua_random_index(candidates.len()) else {
            break;
        };
        current = candidates[index];
        deliveries.push((current, *hop_instances));
    }

    let mut ops = Vec::new();
    for (target_uid, instances) in deliveries {
        ops.push(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::ReserveChildUids(BuffChildUidReservation {
                origin,
                target_uid,
                count: *reserved_children,
            }),
        )));
        ops.extend((0..instances).map(|_| {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantChild(
                BuffGrantChild {
                    origin,
                    source_uid: context.source_uid,
                    target_uid,
                    buff_id: *buff_id,
                    amount: None,
                    params: None,
                    act_info: None,
                },
            )))
        }));
    }
    Some(ops)
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        manager::BattleManagers,
        runtime::determinism::RoundDeterminism,
        skill::{action::SkillModifiers, target::TargetContext},
    };

    #[test]
    fn ramona_settles_poison_at_configured_rate_without_shortening_duration() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam { entitys: vec![FightEntityInfo {
                uid: Some(10), model_id: Some(3142), team_type: Some(1), current_hp: Some(1000),
                attr: Some(sonettobuf::HeroAttribute { hp: Some(1000), attack: Some(1000), ..Default::default() }),
                ..Default::default()
            }], ..Default::default() }),
            defender: Some(FightTeam { entitys: vec![FightEntityInfo {
                uid: Some(-1), team_type: Some(2), current_hp: Some(10000),
                attr: Some(sonettobuf::HeroAttribute { hp: Some(10000), ..Default::default() }),
                ..Default::default()
            }], ..Default::default() }), ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
        let mut events = crate::engine::event::bus::EventBus::default();
        let origin = crate::engine::skill::rule::CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::Behavior,
            key: crate::engine::skill::rule::DefinitionKey::new(1, "AddBuff"),
        };
        crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut events,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                origin, source_uid: 10, target_uid: -1, buff_id: 31420000,
                amount: Some(1), occurrences: 1, child_uid_reservations: 0,
            })))).unwrap();
        let buff_uid = managers.buff.buff_id_uid(-1, 31420000).unwrap();
        let before = managers.buff.snapshot(-1, buff_uid).unwrap().duration;
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::new(60083, "SettleDotAndCostDotDuration", vec![1, 1, 1250]);
        let ops = settle_poison_ops(BehaviorOpContext {
            source_uid: 10, source_team: 1, target_uid: -1, active_skill_id: 31420122,
            transfer_count: 1, event: None, managers: &managers, pool: &pool,
            determinism: &mut determinism, modifiers: &mut modifiers, target: &mut target,
        }, &behavior).unwrap();
        assert!(matches!(ops.as_slice(), [RuleOp::Command(BattleCommand::Hp(
            crate::engine::manager::hp::HpCommand::Lose(loss)))] if loss.amount == 500));
        for op in ops { crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut events, op).unwrap(); }
        assert_eq!(managers.hp.current(-1), 9500);
        assert_eq!(managers.buff.snapshot(-1, buff_uid).unwrap().duration, before);
    }

    #[test]
    fn tuesday_poison_settlement_respects_duration_lock() {
        crate::test_support::init_config();
        for locked in [false, true] {
            let fight = Fight {
                attacker: Some(FightTeam { entitys: vec![FightEntityInfo {
                    uid: Some(10), team_type: Some(1), current_hp: Some(1000),
                    attr: Some(sonettobuf::HeroAttribute { hp: Some(1000), attack: Some(1000), ..Default::default() }),
                    ..Default::default()
                }], ..Default::default() }),
                defender: Some(FightTeam { entitys: vec![FightEntityInfo {
                    uid: Some(-1), team_type: Some(2), current_hp: Some(10000),
                    attr: Some(sonettobuf::HeroAttribute { hp: Some(10000), ..Default::default() }),
                    ..Default::default()
                }], ..Default::default() }), ..Default::default()
            };
            let mut managers = BattleManagers::seeded(&fight);
            let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
            let mut events = crate::engine::event::bus::EventBus::default();
            let origin = crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(1, "AddBuff"),
            };
            for id in if locked { vec![31420000, 30980131] } else { vec![31420000] } {
                crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut events,
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                        origin, source_uid: 10, target_uid: -1, buff_id: id,
                        amount: Some(1), occurrences: 1, child_uid_reservations: 0,
                    })))).unwrap();
            }
            let buff_uid = managers.buff.buff_id_uid(-1, 31420000).unwrap();
            let before = managers.buff.snapshot(-1, buff_uid).unwrap().duration.unwrap();
            let mut determinism = RoundDeterminism::default();
            let mut modifiers = SkillModifiers::default();
            let mut target = TargetContext::default();
            let behavior = ParsedBehavior::new(60073, "SettleDotAndCostDotDuration", vec![1, 0, 1000]);
            let ops = settle_poison_ops(BehaviorOpContext {
                source_uid: 10, source_team: 1, target_uid: -1, active_skill_id: 30980151,
                transfer_count: 1, event: None, managers: &managers, pool: &pool,
                determinism: &mut determinism, modifiers: &mut modifiers, target: &mut target,
            }, &behavior).unwrap();
            for op in ops { crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut events, op).unwrap(); }
            assert_eq!(managers.hp.current(-1), 9600);
            assert_eq!(managers.buff.snapshot(-1, buff_uid).unwrap().duration,
                Some(if locked { before } else { before - 1 }));
        }
    }

    #[test]
    fn catapult_uses_configured_instances_and_stops_without_an_alternate_target() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::new(60074, "CatapultBuff", vec![1, 2, 2, 30980111, 1, 2]);

        let ops = catapult_ops(
            &mut BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 1,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert_eq!(ops.len(), 3);
        assert!(matches!(
            &ops[0],
            RuleOp::Command(BattleCommand::Buff(BuffCommand::ReserveChildUids(
                BuffChildUidReservation {
                    target_uid: -1,
                    count: 1,
                    ..
                }
            )))
        ));
        let mut events = crate::engine::event::bus::EventBus::default();
        for op in ops {
            crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut events, op)
                .unwrap();
        }
        assert_eq!(
            managers.buff.buff_act_amount(
                -1,
                crate::engine::skill::buff_act::registry::BuffActKind::Poison
            ),
            2
        );

        let conversion = ParsedBehavior::new(60110, "PoisonConvertToTargetBuff", vec![1, 31040013]);
        let ops = convert_poison_ops(
            &BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 2,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &conversion,
        )
        .unwrap();
        for op in ops {
            crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut events, op)
                .unwrap();
        }
        assert_eq!(
            managers.buff.buff_act_amount(
                -1,
                crate::engine::skill::buff_act::registry::BuffActKind::Poison
            ),
            1
        );
        assert_eq!(managers.buff.buff_id_amount(-1, 31040013), 1);
    }

    #[test]
    fn powerful_poison_conversion_removes_distinct_instances_before_aggregate_grant() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let poison = ParsedBehavior::new(60074, "CatapultBuff", vec![1, 2, 3, 30980111, 1, 0]);
        let mut context = BehaviorOpContext {
            source_uid: 10,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 1,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        };
        let mut events = crate::engine::event::bus::EventBus::default();
        for op in catapult_ops(&mut context, &poison).unwrap() {
            crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut events, op)
                .unwrap();
        }
        let mut poison_uids = managers
            .buff
            .active_features(&managers.hp)
            .into_iter()
            .filter(|feature| {
                feature.owner_uid == -1
                    && crate::engine::skill::buff_act::is_kind(
                        feature,
                        crate::engine::skill::buff_act::registry::BuffActKind::Poison,
                    )
            })
            .map(|feature| feature.buff_uid)
            .collect::<Vec<_>>();
        poison_uids.sort_unstable();

        let conversion = ParsedBehavior::new(
            60284,
            "PoisonConvertToPowerfulPoisonBuff",
            vec![6, 31420003],
        );
        let ops = super::super::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 2,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &conversion,
        )
        .unwrap();

        assert_eq!(ops.len(), 4);
        for (op, expected_uid) in ops[..3].iter().zip(&poison_uids) {
            assert!(matches!(
                op,
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                    target_uid: -1,
                    selector: BuffRemoveSelector::Uid(uid),
                    ..
                }))) if uid == expected_uid
            ));
        }
        assert!(matches!(
            &ops[3],
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                target_uid: -1,
                buff_id: 31420003,
                amount: Some(3),
                occurrences: 1,
                ..
            })))
        ));
        events = crate::engine::event::bus::EventBus::default();
        for op in ops {
            crate::engine::runtime::executor::execute_rule_op(&mut managers, &mut events, op)
                .unwrap();
        }
        for poison_uid in poison_uids {
            assert!(managers.buff.snapshot(-1, poison_uid).is_none());
        }
        assert_eq!(managers.buff.buff_id_amount(-1, 31420003), 3);
        let output_uid = managers.buff.buff_id_uid(-1, 31420003).unwrap();
        assert_eq!(
            managers.buff.snapshot(-1, output_uid).unwrap().duration,
            Some(2)
        );
    }
}
