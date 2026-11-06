use crate::engine::skill::{
    behavior::{BehaviorOpContext, registry::BehaviorHandler},
    effect::ParsedBehavior,
    rule::output::RuleOp,
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        (behavior.raw_args.is_empty() || behavior.raw_args.len() == 2)
            && matches!(behavior.args.as_slice(), [1, 2] | [1, 3] | [1, 4])
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        Self::supports(behavior).then(|| {
            vec![RuleOp::EffectMarker {
                target_uid: context.target.runtime_target_uid,
                effect_type: sonettobuf::effect_type_enum::EffectType::Unnamedstrengthen as i32,
                effect_num: behavior.args[0],
                effect_num1: Some(behavior.args[1]),
                config_effect: 0,
                reserve_id: None,
                reserve_str: None,
            }]
        })
    }
}
