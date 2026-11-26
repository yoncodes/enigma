use super::*;
use crate::engine::{
    entity::attr::AttrId,
    manager::ex_point::ExPointCommand,
    skill::{action::SkillLifecycle, condition::registry::PER_DECR_EX_POINT_KEY},
};

pub(super) fn payer(command: &RuleOp, catalog: &SkillEffectCatalog) -> Option<i64> {
    let (source_uid, skill_id, cost) = match command {
        RuleOp::BeginSkillAction {
            lifecycle: SkillLifecycle::PhaseCompleted(action),
            cost,
        } => (action.source_uid, action.skill_id, cost),
        RuleOp::PaySkillAction { action, cost } => (action.source_uid, action.skill_id, cost),
        _ => return None,
    };
    let ExPointCommand::Spend(cost) = cost else {
        return None;
    };
    (cost.target_uid == source_uid && catalog.is_big_skill(skill_id) && catalog.is_attack(skill_id))
        .then_some(source_uid)
}

fn is_own_spend_reaction(op: &QueuedOp, source_uid: i64) -> bool {
    let SkillOpTrigger::Event(event) = &op.trigger else {
        return false;
    };
    event.moxie_spender() == Some(source_uid)
        && matches!(&op.op, RuleOp::Skill(invocation)
            if invocation.condition_key == Some(PER_DECR_EX_POINT_KEY))
}

pub(super) fn settle(
    access: DrainAccess<'_>,
    payer: Option<i64>,
    mut reactions: VecDeque<QueuedOp>,
) -> Result<i32, DrainError> {
    let DrainAccess {
        managers,
        pool,
        catalog,
        determinism,
        result,
        state,
    } = access;
    let Some(payer) = payer else {
        if !reactions.is_empty() {
            drain_nested_queue(
                managers,
                pool,
                catalog,
                determinism,
                reactions,
                result,
                state,
            )?;
        }
        return Ok(0);
    };
    let is_credit = |op: &QueuedOp| is_own_spend_reaction(op, payer);
    let mut credit = 0i32;
    while !reactions.is_empty() {
        let reads_credit = is_credit(&reactions[0]);
        let boundary = reactions
            .iter()
            .position(|op| is_credit(op) != reads_credit)
            .unwrap_or(reactions.len());
        let tail = reactions.split_off(boundary);
        let current = std::mem::replace(&mut reactions, tail);
        let sample = |managers: &BattleManagers| {
            if reads_credit {
                managers.origin_attribute(payer, AttrId::CriticalDmg)
            } else {
                0
            }
        };
        let before = sample(managers);
        drain_nested_queue(managers, pool, catalog, determinism, current, result, state)?;
        credit = credit.saturating_add(sample(managers).saturating_sub(before));
    }
    Ok(credit)
}
