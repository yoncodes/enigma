use crate::engine::{
    damage::{handler::modified_heal, scale_permille},
    manager::hp::{HpCommand, HpHeal, HpHealKind},
    skill::{
        behavior::{
            BehaviorOpContext,
            registry::{BehaviorHandler, OutputOwner},
        },
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        matches!(behavior.args.as_slice(), [rate, 0] if *rate > 0)
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [rate, 0] = behavior.args.as_slice() else {
            return None;
        };
        let origin = super::command_origin(behavior)?;
        let mut ops = Vec::new();
        for target_uid in context
            .pool
            .main_allies(context.source_uid)
            .iter()
            .map(|target| target.uid)
        {
            let state = crate::engine::skill::buff_act::lost_hp_to_fake_hp::state(
                context.managers,
                target_uid,
            )
            .ok()?;
            let Some(state) = state.filter(|state| state.current > 0) else {
                continue;
            };
            let amount = modified_heal(
                scale_permille(state.current, *rate),
                context.source_uid,
                target_uid,
                context.managers,
                &context.modifiers.attack_attributes,
            );
            if amount <= 0 {
                continue;
            }
            ops.extend([
                RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(HpHeal {
                    origin,
                    source_uid: context.source_uid,
                    target_uid,
                    amount,
                    config_effect: behavior.config_effect,
                    kind: HpHealKind::Normal,
                }))),
                RuleOp::BuffActInfoMarker(crate::engine::manager::buff::BuffActInfoMarkerResult {
                    target_uid,
                    buff_uid: state.buff_uid,
                    act_id: state.act_id,
                    params: vec![state.current, state.maximum],
                    str_param: Some(String::new()),
                    team_type: 0,
                }),
            ]);
        }
        Some(ops)
    }

    fn output_owner(_: &ParsedBehavior, op: &RuleOp, _: usize) -> Option<OutputOwner> {
        matches!(op, RuleOp::BuffActInfoMarker(_)).then_some(OutputOwner::Parent)
    }
}

#[cfg(test)]
mod tests;
