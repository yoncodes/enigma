use crate::engine::{
    manager::upgrade::{UpgradeCommand, UpgradeOperation},
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub fn rule_op(target_uid: i64, behavior: &ParsedBehavior) -> Option<RuleOp> {
    if behavior.spec.kind != BehaviorKind::NotifyUpgradeHero {
        return None;
    }
    Some(RuleOp::Command(BattleCommand::Upgrade(UpgradeCommand {
        owner_uid: target_uid,
        operation: UpgradeOperation::Offer {
            origin: super::command_origin(behavior)?,
            upgrade_id: behavior.arg(0)?,
        },
    })))
}

pub(super) fn supports_notify_upgrade(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [upgrade_id] if *upgrade_id > 0)
        && behavior.raw_args.len() <= 1
}

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if behavior.spec.kind == BehaviorKind::NotifyUpgradeHero {
            let upgrade_id = behavior.arg(0)?;
            let entity = context.managers.entity_snapshot(context.target_uid)?;
            let selected = entity
                .enhance_info_box
                .as_ref()
                .map(|info| info.upgraded_options.as_slice())
                .unwrap_or_default();
            if !context
                .managers
                .catalog()
                .upgrade_has_available_option(upgrade_id, selected)?
            {
                return Some(Vec::new());
            }
        }
        rule_op(context.target_uid, behavior).map(|op| vec![op])
    }
}

pub(super) struct AssassinateHandler;

pub(super) struct AddMaxRoundHandler;

pub(super) struct ImmediatelyMonsterHandler;

impl BehaviorHandler for ImmediatelyMonsterHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        behavior.args.is_empty()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        Self::supports(behavior).then(|| {
            vec![RuleOp::Command(BattleCommand::ImmediateReservePromotion(
                crate::engine::manager::battle_rule::ImmediateReservePromotionCommand {
                    team_type: context.source_team,
                },
            ))]
        })
    }
}

impl BehaviorHandler for AddMaxRoundHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(behavior.args.as_slice(), [increment, cap] if *increment > 0 && *cap > 0)
    }

    fn emit_ops(_: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        Self::supports(behavior).then(|| {
            vec![RuleOp::Command(BattleCommand::MaxRound(
                crate::engine::manager::battle_rule::MaxRoundCommand {
                    increment: behavior.args[0],
                    cap: behavior.args[1],
                },
            ))]
        })
    }
}

pub(super) struct AddIndicatorHandler;

impl BehaviorHandler for AddIndicatorHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(behavior.args.as_slice(), [indicator_id, _] if *indicator_id > 0)
    }

    fn emit_ops(_: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        Self::supports(behavior).then(|| {
            vec![RuleOp::Command(BattleCommand::Indicator(
                crate::engine::manager::indicator::IndicatorCommand {
                    indicator_id: behavior.args[0],
                    amount: behavior.args[1],
                    config_effect: behavior.spec.key.opcode,
                },
            ))]
        })
    }
}

pub(super) struct DelIndicatorHandler;

impl BehaviorHandler for DelIndicatorHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(behavior.args.as_slice(), [indicator_id] if *indicator_id > 0)
    }

    fn emit_ops(_: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        Self::supports(behavior).then(|| {
            vec![RuleOp::Command(BattleCommand::IndicatorClear(
                crate::engine::manager::indicator::IndicatorClearCommand {
                    indicator_id: behavior.args[0],
                    config_effect: behavior.spec.key.opcode,
                },
            ))]
        })
    }
}

impl BehaviorHandler for AssassinateHandler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if behavior.args.is_empty() {
            context.target.active_skill_assassinate = true;
            Some(Vec::new())
        } else {
            None
        }
    }
}

pub fn supports_extra_type(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [1 | 3])
}

pub(super) struct SetExtraTypeHandler;

impl BehaviorHandler for SetExtraTypeHandler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        context.target.extra_skill_kind =
            super::super::condition::extra::skill_kind_from_is_extra(behavior.arg(0)?)?.id();
        Some(Vec::new())
    }
}

pub(super) struct DamageRateMarkerHandler;

impl BehaviorHandler for DamageRateMarkerHandler {
    fn emit_ops(_: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        (behavior.args.is_empty()
            && behavior.spec.kind == BehaviorKind::IgnoreSkillConfigDamageRate)
            .then(Vec::new)
    }
}

#[cfg(test)]
mod tests;
