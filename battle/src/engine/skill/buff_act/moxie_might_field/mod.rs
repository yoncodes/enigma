use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        buff::{
            ActiveBuffFeature, BuffActInfoMarkerResult, BuffCommand, BuffManager, BuffSetState,
        },
        hp::HpManager,
        moxie_field::Credit,
    },
    skill::{
        action::SkillPhase,
        buff_act::{
            BuffActRuleOp,
            registry::{BuffActKind, RuntimeContext},
        },
        rule::output::{BattleCommand, RuleOp},
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [attr, 300, 15, 20] | [attr, 400, 20, 20] | [attr, 500, 25, 20]
        if AttrId::from_raw(*attr) == Some(AttrId::UltimateMight))
}

pub fn team_might(
    buffs: &BuffManager,
    hp: &HpManager,
    features: &[ActiveBuffFeature],
    uid: i64,
    attr: AttrId,
) -> i32 {
    if attr != AttrId::UltimateMight || hp.current(uid) <= 0 {
        return 0;
    }
    let Some(team) = buffs.team_type(uid) else {
        return 0;
    };
    features
        .iter()
        .filter(|feature| {
            feature.owner_alive
                && feature.team_type == team
                && super::is_kind(feature, BuffActKind::MoxieMightField)
        })
        .filter_map(|feature| {
            let [act, _, base, rate, cap] = feature.values.as_slice() else {
                return None;
            };
            let held = buffs.snapshot(feature.owner_uid, feature.buff_uid)?;
            let state = held
                .act_info
                .iter()
                .find(|state| state.act_id == Some(*act))?;
            let count = *state.param.first()?;
            Some(base.saturating_add(rate.saturating_mul(count.clamp(0, *cap))))
        })
        .fold(0i32, i32::saturating_add)
}

pub fn rule_ops(context: &mut RuntimeContext<'_>) -> Option<Vec<BuffActRuleOp>> {
    let subscriber = context.subscriber;
    let managers = context.managers;
    if !subscriber.owner_alive {
        return Some(Vec::new());
    }
    let [_, _, _, cap] = subscriber.args.as_slice() else {
        return None;
    };
    let origin = super::command_origin(subscriber)?;
    let act_id = subscriber.key.definition.opcode;
    let owner_uid = subscriber.owner_uid;
    let buff_uid = subscriber.buff_uid;
    let mut held = managers.buff.snapshot(owner_uid, buff_uid)?;
    let state = held
        .act_info
        .iter_mut()
        .find(|state| state.act_id == Some(act_id))?;
    let [published] = state.param.as_mut_slice() else {
        return None;
    };
    let current = managers
        .moxie_field
        .amount(owner_uid, buff_uid)
        .unwrap_or(*published);
    match context.event? {
        BattleEvent::UltimateMoxiePaid(payment) if payment.team == subscriber.team_type => {
            let amount = current.saturating_add(payment.amount).min(*cap);
            if amount == current {
                return Some(Vec::new());
            }
            Some(vec![BuffActRuleOp::causing(RuleOp::Command(
                BattleCommand::MoxieFieldCredit(Credit {
                    owner_uid,
                    buff_uid,
                    amount,
                }),
            ))])
        }
        BattleEvent::SkillEffectsPrepared(action)
            if action.phase == SkillPhase::Immediate
                && action.mode.publishes_completion()
                && context.pool.team_type(action.source_uid) == Some(subscriber.team_type)
                && current > 0 =>
        {
            let mut ops = Vec::new();
            if *published != current {
                *published = current;
                ops.push(BuffActRuleOp::subscriber_from_owner(RuleOp::Command(
                    BattleCommand::Buff(BuffCommand::SetInternalState(BuffSetState {
                        origin,
                        target_uid: owner_uid,
                        buff_uid,
                        ex_info: None,
                        params: None,
                        act_info: Some(held.act_info),
                    })),
                )));
            }
            ops.push(BuffActRuleOp::subscriber_from_owner(
                RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
                    target_uid: owner_uid,
                    buff_uid,
                    act_id,
                    params: vec![current],
                    str_param: Some(String::new()),
                    team_type: 0,
                }),
            ));
            Some(ops)
        }
        _ => Some(Vec::new()),
    }
}

#[cfg(test)]
mod tests;
