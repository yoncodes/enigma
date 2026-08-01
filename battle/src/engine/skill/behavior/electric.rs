use crate::engine::{
    manager::buff::{BuffSelector, CommandOrigin},
    mechanic::field_transfer::FieldTransferCommand,
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

pub(super) fn supports(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [buff_type_id, max_count, 1] if *buff_type_id > 0 && *max_count > 0)
}

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        transfer_rule_ops(context, behavior)
    }
}

fn transfer_rule_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    if behavior.spec.kind != BehaviorKind::ElectricTransform {
        return None;
    }
    if !supports(behavior) {
        return Some(Vec::new());
    }
    let buff_type_id = behavior.args[0];
    let max_count = behavior.args[1];
    let amount = context
        .managers
        .buff
        .buff_id_or_type_amount(context.target_uid, buff_type_id)
        .min(max_count);
    if amount <= 0
        || context
            .managers
            .field
            .get(context.source_team)
            .is_none_or(|field| field.next_upgrade_progress <= 0)
    {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::FieldTransfer(
        FieldTransferCommand {
            origin: CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: super::registry::find(behavior)?.key,
            },
            target_uid: context.target_uid,
            buff: BuffSelector::IdOrType(buff_type_id),
            limit: max_count,
            team: context.source_team,
        },
    ))])
}
