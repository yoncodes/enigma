use std::collections::BTreeMap;

use sonettobuf::{BeginRoundOper, FightDeviceOper};

use crate::engine::{
    diagnostics::{self, TraceArea},
    manager::{
        BattleManagers,
        card::{CardOpType, PlayedCard},
        conduit::{ConduitError, Payment},
    },
    runtime::{conduit::admit, determinism::RoundDeterminism},
    skill::{
        behavior::resource::fixed_conduit_gain,
        effect::SkillEffectCatalog,
        target::{TargetContext, TargetPool},
    },
};

#[cfg(test)]
mod tests;

type Powers = BTreeMap<(i32, i32), i32>;
type Spend = Result<[(i32, i32); 2], ConduitError>;

struct Estimate {
    current: Powers,
    queued: Powers,
    first_pass: Vec<(i64, i32, Spend)>,
}

fn enabled(devices: &[FightDeviceOper]) -> bool {
    !devices.is_empty() && diagnostics::enabled(TraceArea::Conduit)
}

pub(in crate::engine::auto_battle) fn unavailable(
    round: i32,
    devices: &[FightDeviceOper],
    reason: &str,
) {
    if enabled(devices) {
        eprintln!("conduit demand round={round} unavailable: {reason}");
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::engine::auto_battle) fn trace(
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    context: TargetContext,
    devices: &[FightDeviceOper],
    prefix: &[BeginRoundOper],
    played: &[PlayedCard],
) {
    if !enabled(devices) {
        return;
    }
    match estimate(
        managers,
        pool,
        catalog,
        determinism,
        context,
        devices,
        prefix,
        played,
    ) {
        Ok(estimate) => eprintln!(
            "conduit demand round={} current={:?} queued_fixed={:?} first_pass={:?}",
            context.current_round, estimate.current, estimate.queued, estimate.first_pass
        ),
        Err(reason) => eprintln!(
            "conduit demand round={} unavailable: {reason}",
            context.current_round
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn estimate(
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    context: TargetContext,
    devices: &[FightDeviceOper],
    prefix: &[BeginRoundOper],
    played: &[PlayedCard],
) -> Result<Estimate, &'static str> {
    if prefix.iter().any(|op| {
        op.oper_type == Some(CardOpType::PlayCard.id()) && op.param2.is_some_and(|index| index > 0)
    }) {
        return Err("unresolved discard in prefix");
    }
    if prefix
        .iter()
        .any(|op| op.oper_type == Some(CardOpType::AssistBoss.id()))
    {
        return Err("unverified assist in prefix");
    }
    let mut queued = Powers::new();
    for card in played {
        if card.card.skill_id != Some(card.skill_id)
            && !super::super::card_choice_options(&card.card, managers).contains(&card.skill_id)
        {
            return Err("unverified card choice");
        }
        let Some(team) = pool.team_type(card.caster_uid) else {
            continue;
        };
        for (power, gain) in catalog
            .get(card.skill_id)
            .into_iter()
            .flat_map(|effect| &effect.slots)
            .filter_map(fixed_conduit_gain)
        {
            let value = queued.entry((team, power)).or_default();
            *value = value.saturating_add(gain);
        }
    }
    let current: Powers = managers.conduit.powers().collect();
    let mut available = current.clone();
    for (&key, &gain) in &queued {
        let value = available.entry(key).or_default();
        *value = value.saturating_add(gain);
    }
    let mut first_pass = Vec::new();
    for operation in devices {
        let (Some(uid), Some(group)) = (operation.uid, operation.index) else {
            continue;
        };
        let Some(team) = pool.team_type(uid) else {
            continue;
        };
        let Some(skills) = managers
            .conduit
            .devices()
            .find(|device| device.uid == uid)
            .and_then(|device| device.skill_groups.get(group.saturating_sub(1) as usize))
        else {
            continue;
        };
        for &skill in skills {
            if catalog.get(skill.skill_id).is_none() || !catalog.issues(skill.skill_id).is_empty() {
                continue;
            }
            let Some(admission) = admit(
                managers,
                pool,
                catalog,
                determinism,
                context,
                uid,
                skill,
                false,
            )
            .filter(|admission| admission.runtime_ready && admission.selectable) else {
                continue;
            };
            let payment = Payment::new(skill, admission.cost);
            if !payment.uses_energy() {
                continue;
            }
            let spend =
                payment.split(|power| available.get(&(team, power)).copied().unwrap_or_default());
            if let Ok(split) = spend {
                for (power, amount) in split {
                    if amount > 0 {
                        *available.entry((team, power)).or_default() -= amount;
                    }
                }
            }
            first_pass.push((uid, skill.skill_id, spend));
        }
    }
    Ok(Estimate {
        current,
        queued,
        first_pass,
    })
}
