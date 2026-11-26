use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        buff::{
            ActiveBuffFeature, BuffManager,
            moxie_field::{Credit, Sample},
        },
        hp::HpManager,
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
            let [_, _, base, rate, cap] = feature.values.as_slice() else {
                return None;
            };
            let count = buffs
                .moxie_field(feature.owner_uid, feature.buff_uid)?
                .sampled;
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
    let owner_uid = subscriber.owner_uid;
    let buff_uid = subscriber.buff_uid;
    let current = managers.buff.moxie_field(owner_uid, buff_uid)?.credited;
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
            Some(vec![BuffActRuleOp::subscriber_from_owner(RuleOp::Command(
                BattleCommand::MoxieFieldSample(Sample {
                    owner_uid,
                    buff_uid,
                }),
            ))])
        }
        _ => Some(Vec::new()),
    }
}

#[cfg(test)]
mod tests;
