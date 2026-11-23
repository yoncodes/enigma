use crate::engine::skill::{
    behavior::{BehaviorOpContext, registry::BehaviorHandler},
    effect::ParsedBehavior,
    rule::output::RuleOp,
};

pub(crate) const COPY_PREVIEW: i32 = 5;

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let effect_num = behavior.arg(0)?;
        let target_uid = context.target_uid;
        let operation = if behavior.raw_arity == 1 && effect_num == COPY_PREVIEW {
            RuleOp::CopyPreview { target_uid }
        } else {
            RuleOp::EffectMarker {
                target_uid,
                effect_type: sonettobuf::effect_type_enum::EffectType::Clienteffect as i32,
                effect_num,
                effect_num1: None,
                config_effect: behavior.spec.key.opcode,
                reserve_id: None,
                reserve_str: None,
            }
        };
        Some(vec![operation])
    }
}
