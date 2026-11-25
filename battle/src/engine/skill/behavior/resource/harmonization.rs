use crate::engine::{
    manager::ex_point::{ExPointChange, ExPointCommand, ExPointKind},
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};
use sonettobuf::effect_type_enum::EffectType;

pub(in crate::engine::skill::behavior) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let origin = super::super::command_origin(behavior)?;
        let delta = behavior.arg(0)?.saturating_mul(context.transfer_count);
        let target_uid = context.target_uid;
        if ExPointKind::from_wire(context.managers.ex_point.kind(target_uid))
            != ExPointKind::DevicePower
        {
            return Some(Vec::new());
        }
        Some(vec![RuleOp::Command(BattleCommand::ExPoint(
            ExPointCommand::Change(ExPointChange {
                origin,
                source_uid: context.source_uid,
                target_uid,
                delta,
                config_effect: 0,
                effect_type: EffectType::Expointchange as i32,
            }),
        ))])
    }
}
