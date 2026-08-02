use crate::engine::{
    manager::buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
    skill::{
        action::SkillRateModifier,
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if behavior.spec.kind == BehaviorKind::MustCrit {
            context.modifiers.force_critical = true;
            return Some(Vec::new());
        }
        if behavior.spec.kind == BehaviorKind::IgnoreBeatBack {
            context.modifiers.ignore_riposte = true;
            return Some(Vec::new());
        }
        if behavior.spec.kind == BehaviorKind::ConsumeBuffFixMixedRate {
            return mixed_rate_ops(context, behavior);
        }
        if behavior.spec.kind == BehaviorKind::ConsumeBuffAttrFix {
            let [type_or_buff_id, amount, raw_attr_id, delta] = behavior.args.as_slice() else {
                return None;
            };
            let mut ops = Vec::new();
            if *amount > 0 {
                ops.push(RuleOp::Command(BattleCommand::Buff(
                    BuffCommand::ConsumeCoalesced(BuffConsume {
                        origin: super::command_origin(behavior)?,
                        target_uid: context.target_uid,
                        selector: BuffSelector::IdOrType(*type_or_buff_id),
                        amount: *amount,
                        depleted: DepletedBuff::Remove,
                    }),
                )));
            }
            if *delta != 0 {
                context.modifiers.attack_attributes.push((
                    crate::engine::entity::attr::AttrId::from_raw(*raw_attr_id)?,
                    *delta,
                ));
            }
            return Some(ops);
        }
        let [type_or_buff_id, amount, value] = behavior.args.as_slice() else {
            return None;
        };
        if !matches!(
            behavior.spec.kind,
            BehaviorKind::ConsumeBuffUpSkillDamageRate | BehaviorKind::ConsumeBuffChangeTargets
        ) {
            return None;
        }
        let mut ops = Vec::new();
        if *amount > 0 {
            ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
                BuffConsume {
                    origin: super::command_origin(behavior)?,
                    target_uid: context.target_uid,
                    selector: BuffSelector::IdOrType(*type_or_buff_id),
                    amount: *amount,
                    depleted: DepletedBuff::Remove,
                },
            ))));
        }
        match behavior.spec.kind {
            BehaviorKind::ConsumeBuffUpSkillDamageRate if *value != 0 => {
                context.modifiers.rates.push(SkillRateModifier::fixed(
                    0,
                    behavior.spec.key.opcode,
                    *value,
                    true,
                ));
            }
            BehaviorKind::ConsumeBuffChangeTargets if *value > 0 => {
                ops.push(RuleOp::ModifyActiveSkillTargets {
                    additional_count: *value,
                });
            }
            _ => {}
        }
        Some(ops)
    }
}

fn mixed_rate_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [
        buff_id,
        amount,
        raw_attr_id,
        attr_delta,
        rate,
        conditional_rate,
        conditional_buff_id,
    ] = behavior.args.as_slice()
    else {
        return None;
    };
    let attr = crate::engine::entity::attr::AttrId::from_raw(*raw_attr_id)?;
    if *attr_delta != 0 {
        context
            .modifiers
            .attack_attributes
            .push((attr, *attr_delta));
    }
    if context
        .managers
        .buff
        .buff_id_amount(context.target_uid, *buff_id)
        < *amount
    {
        return Some(Vec::new());
    }
    if *attr_delta != 0 {
        context
            .modifiers
            .attack_attributes
            .push((attr, *attr_delta));
    }
    if *rate != 0 {
        context.modifiers.rates.push(SkillRateModifier::fixed(
            0,
            behavior.spec.key.opcode,
            *rate,
            true,
        ));
    }
    if *conditional_rate != 0 {
        context.modifiers.rates.extend(
            context
                .pool
                .enemies(context.source_uid, false)
                .iter()
                .filter(|target| {
                    context
                        .managers
                        .buff
                        .buff_id_or_type_amount(target.uid, *conditional_buff_id)
                        > 0
                })
                .map(|target| {
                    SkillRateModifier::fixed(
                        target.uid,
                        behavior.spec.key.opcode,
                        *conditional_rate,
                        true,
                    )
                }),
        );
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::ConsumeCoalesced(BuffConsume {
            origin: super::command_origin(behavior)?,
            target_uid: context.target_uid,
            selector: BuffSelector::ExactId(*buff_id),
            amount: *amount,
            depleted: DepletedBuff::Remove,
        }),
    ))])
}

pub(super) fn supports_mixed_rate(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [buff_id, amount, raw_attr_id, attr_delta, rate, conditional_rate, conditional_buff_id]
            if *buff_id > 0
                && *amount > 0
                && crate::engine::entity::attr::AttrId::from_raw(*raw_attr_id).is_some()
                && *attr_delta >= 0
                && *rate >= 0
                && *conditional_rate >= 0
                && *conditional_buff_id > 0
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        manager::BattleManagers,
        runtime::determinism::RoundDeterminism,
        skill::{
            action::SkillModifiers,
            behavior::{self, classify::BehaviorSpec},
            target::{TargetContext, TargetPool},
        },
    };

    #[test]
    fn consume_rate_emits_state_change_and_keeps_rate_cast_local() {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60034, "ConsumeBuffUpSkillDamageRate"),
            vec![30631, 1, 250],
            Vec::new(),
        );

        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 100,
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

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
                BuffConsume {
                    selector: BuffSelector::IdOrType(30631),
                    amount: 1,
                    ..
                }
            )))]
        ));
        assert_eq!(modifiers.rates[0].fixed_value(), Some(250));

        modifiers = SkillModifiers::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60035, "ConsumeBuffAttrFix"),
            vec![
                8178,
                4,
                crate::engine::entity::attr::AttrId::CriticalDmg as i32,
                500,
            ],
            Vec::new(),
        );
        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 100,
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
        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Buff(
                BuffCommand::ConsumeCoalesced(BuffConsume { amount: 4, .. })
            ))]
        ));
        assert_eq!(
            modifiers.attack_attributes,
            vec![(crate::engine::entity::attr::AttrId::CriticalDmg, 500)]
        );
    }

    #[test]
    fn consumed_stacks_add_the_configured_attribute_and_target_rates() {
        crate::test_support::init_config();
        let fight = sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(100),
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(20),
                        buff_id: Some(30650201),
                        layer: Some(2),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(-1),
                    team_type: Some(2),
                    current_hp: Some(100),
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(21),
                        buff_id: Some(30650204),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(100019, "ConsumeBuffFixMixedRate"),
            vec![30650201, 2, 213, 300, 4500, 0, 30650104],
            Vec::new(),
        );

        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 30650213,
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

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Buff(
                BuffCommand::ConsumeCoalesced(BuffConsume {
                    selector: BuffSelector::ExactId(30650201),
                    amount: 2,
                    ..
                })
            ))]
        ));
        assert_eq!(
            modifiers.attack_attributes,
            vec![
                (crate::engine::entity::attr::AttrId::Penetration, 300),
                (crate::engine::entity::attr::AttrId::Penetration, 300),
            ]
        );
        assert_eq!(
            modifiers
                .rates
                .iter()
                .map(|modifier| (modifier.target_uid, modifier.fixed_value()))
                .collect::<Vec<_>>(),
            vec![(0, Some(4500))]
        );

        modifiers = SkillModifiers::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(100019, "ConsumeBuffFixMixedRate"),
            vec![30650201, 2, 213, 0, 3000, 2250, 30650104],
            Vec::new(),
        );
        behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 30650223,
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
        assert!(modifiers.attack_attributes.is_empty());
        assert_eq!(
            modifiers
                .rates
                .iter()
                .map(|modifier| (modifier.target_uid, modifier.fixed_value()))
                .collect::<Vec<_>>(),
            vec![(0, Some(3000)), (-1, Some(2250))]
        );
    }

    #[test]
    fn must_crit_sets_a_typed_cast_modifier() {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior =
            ParsedBehavior::from_spec(BehaviorSpec::new(60069, "MustCrit"), Vec::new(), Vec::new());

        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: -1,
                active_skill_id: 100,
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

        assert!(ops.is_empty());
        assert!(modifiers.force_critical);
    }

    #[test]
    fn change_targets_emits_an_active_cast_modifier() {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60036, "ConsumeBuffChangeTargets"),
            vec![370002190, 1, 1],
            Vec::new(),
        );

        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: -1,
                source_team: 2,
                target_uid: -1,
                active_skill_id: 370001002,
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

        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(_))),
                RuleOp::ModifyActiveSkillTargets {
                    additional_count: 1
                }
            ]
        ));
        assert_eq!(target.additional_skill_target_count, 0);
    }

    #[test]
    fn ignore_beat_back_marks_only_the_current_skill() {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60054, "IgnoreBeatBack"),
            Vec::new(),
            Vec::new(),
        );

        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 30861141,
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

        assert!(ops.is_empty());
        assert!(modifiers.ignore_riposte);
    }
}
