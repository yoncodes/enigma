use std::collections::HashMap;

use super::{BattleManagers, buff::BuffPlan, hp};
use crate::engine::skill::buff_act::{self, registry::BuffActKind};

pub(super) struct BalancePlan {
    pub reset: BuffPlan,
    pub remove: BuffPlan,
    pub marker: super::buff::BuffActInfoMarkerResult,
}

pub(super) fn prepare(
    managers: &BattleManagers,
    hp: &hp::HpManager,
    target_count: usize,
    shared_values: &HashMap<i64, i32>,
    share_spent: &HashMap<i64, i32>,
    command: &mut hp::HpCommand,
) -> Result<Option<BalancePlan>, hp::HpCommandError> {
    let hp::HpCommand::Damage(hit) = command else {
        return Ok(None);
    };
    if hit.hurt.damage_from != hp::HurtDamageFromType::Skill
        || !matches!(
            hit.effect_kind,
            hp::DamageEffectKind::Normal | hp::DamageEffectKind::Critical
        )
    {
        return Ok(None);
    }
    let Some(mitigation) = buff_act::ripple_balance::mitigate(managers, hit.target_uid, hit.amount)
        .map_err(|_| hp::HpCommandError::InvalidRippleBalance)?
    else {
        return Ok(None);
    };
    if other_mitigation(managers, hp, hit, target_count, shared_values, share_spent) {
        return Err(hp::HpCommandError::UnsupportedRippleInteraction(
            hit.target_uid,
        ));
    }
    hit.amount = mitigation.damage;
    let Some(clear) = mitigation.clear else {
        return Ok(None);
    };
    let plan = |command| {
        managers
            .plan_buff(command)
            .map_err(|_| hp::HpCommandError::InvalidRippleBalance)
    };
    Ok(Some(BalancePlan {
        reset: plan(clear.reset)?,
        remove: plan(clear.remove)?,
        marker: clear.marker,
    }))
}

fn other_mitigation(
    managers: &BattleManagers,
    hp: &hp::HpManager,
    hit: &hp::HpDamage,
    target_count: usize,
    shared_values: &HashMap<i64, i32>,
    share_spent: &HashMap<i64, i32>,
) -> bool {
    let uid = hit.target_uid;
    let active = |kind| managers.buff.has_buff_act_kind(uid, kind);
    hp.shield(uid) > 0
        || managers
            .team_shared_shield_for_hit(hp, uid, target_count, shared_values)
            .and_then(|shield| shield.absorption(hit.amount))
            .is_some()
        || (active(BuffActKind::ShareHurt)
            && buff_act::share_hurt::plan(&managers.buff, hp, &managers.entity, hit, share_spent)
                .is_some())
        || (active(BuffActKind::DamageNotMoreThan)
            && managers
                .damage_cap(hp, uid)
                .is_some_and(|cap| cap < hit.amount))
        || (active(BuffActKind::FoundationShield)
            && buff_act::foundation::mitigate(managers, uid, hit.amount).map_or(true, |result| {
                result.is_some_and(|change| change.prevented > 0)
            }))
        || (active(BuffActKind::LockHp)
            && buff_act::lock_hp::max_loss(&managers.buff, hp, uid)
                .is_some_and(|limit| limit < hit.amount))
        || (active(BuffActKind::LostHpToFakeHp)
            && hit.amount >= hp.current(uid)
            && buff_act::lost_hp_to_fake_hp::plan_damage(managers, uid, hit.amount)
                .map_or(true, |result| {
                    result.is_some_and(|plan| plan.update.is_some())
                }))
}
