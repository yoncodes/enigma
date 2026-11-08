use crate::engine::{
    damage::{
        handler::{crit_chance, crit_heal_multiplier, modified_heal},
        scale_permille,
    },
    entity::attr::AttrId,
    manager::{
        buff::{BuffCommand, BuffGrant},
        hp::{HpCommand, HpHeal, HpHealKind},
    },
    skill::{
        behavior::{BehaviorOpContext, command_origin, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub(super) struct Handler;

pub(super) fn supports(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.args.as_slice(),
        [threshold, 0 | 1, attr, rate, buff_id]
            if (0..=1000).contains(threshold)
                && AttrId::from_raw(*attr).is_some()
                && *rate > 0
                && *buff_id > 0
    )
}

impl BehaviorHandler for Handler {
    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences {
            buffs: behavior.args.get(4).copied().into_iter().collect(),
            ..Default::default()
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [threshold, mode, raw_attr, rate, buff_id] = behavior.args.as_slice() else {
            return None;
        };
        let hp = context.managers.hp.get(context.target_uid);
        let heals = hp.max > 0
            && i64::from(hp.current.max(0)) * 1000 <= i64::from(hp.max) * i64::from(*threshold);
        let origin = command_origin(behavior)?;
        if !heals {
            return Some(vec![RuleOp::Command(BattleCommand::Buff(
                BuffCommand::Grant(BuffGrant {
                    origin,
                    source_uid: context.source_uid,
                    target_uid: context.target_uid,
                    buff_id: *buff_id,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                }),
            ))]);
        }

        let attr = AttrId::from_raw(*raw_attr)?;
        let basis_uid = if *mode == 0 {
            context.source_uid
        } else {
            context.target_uid
        };
        let is_crit = context.determinism.roll_hidden_crit(
            context.active_skill_id,
            context.source_uid,
            context.target_uid,
            crit_chance(
                context.source_uid,
                context.target_uid,
                context.pool,
                context.managers,
            ),
        );
        let mut amount = modified_heal(
            scale_permille(context.managers.origin_attribute(basis_uid, attr), *rate),
            context.source_uid,
            context.target_uid,
            context.managers,
            &context.modifiers.attack_attributes,
        );
        if is_crit {
            amount = crit_heal_multiplier(
                context.source_uid,
                context.target_uid,
                context.pool,
                context.managers,
            )
            .apply(amount);
        }
        Some(if amount > 0 {
            vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(
                HpHeal {
                    origin,
                    source_uid: context.source_uid,
                    target_uid: context.target_uid,
                    amount,
                    config_effect: behavior.config_effect,
                    kind: if is_crit {
                        HpHealKind::Critical
                    } else {
                        HpHealKind::Normal
                    },
                },
            )))]
        } else {
            Vec::new()
        })
    }
}

#[cfg(test)]
mod tests;
