use std::collections::HashSet;

use crate::engine::{
    manager::{BattleManagers, card::CardManager},
    mechanic::{card::CardMechanic, hedone_device_power},
    skill::{effect::SkillEffectCatalog, target::TargetPool},
};

#[derive(Debug)]
pub(crate) enum CardEnergyError {
    Hedone(hedone_device_power::HedoneDevicePowerError),
}

#[derive(Clone, Copy)]
pub(in crate::engine::auto_battle) struct CardEnergy {
    amount: i32,
    special: bool,
    flexible: bool,
    preferred: bool,
}

impl CardEnergy {
    pub fn selectable(self) -> bool {
        self.amount > 0 && self.preferred
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::engine::auto_battle) struct Priority {
    preferred: bool,
    device: bool,
    temporary_special: bool,
    amount: i32,
    special: bool,
    flexible_basic: bool,
    lineup: Option<i32>,
}

impl Priority {
    pub fn new(energy: Option<CardEnergy>, device: bool, temporary: bool, position: i32) -> Self {
        let special = energy.is_some_and(|energy| energy.special);
        Self {
            preferred: energy.is_some_and(|energy| energy.preferred),
            device,
            temporary_special: temporary && special,
            amount: energy.map_or(0, |energy| energy.amount),
            special,
            flexible_basic: !special && energy.is_some_and(|energy| energy.flexible),
            lineup: (device && !special).then_some(position),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(in crate::engine::auto_battle) fn card_energy(
    cards: &CardManager,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    source_uid: i64,
    skill_id: i32,
    temporary: bool,
    selected_power_types: &HashSet<(i32, i32)>,
) -> Result<Option<CardEnergy>, CardEnergyError> {
    let Some(energy) = cards.conduit_skill_energy(skill_id) else {
        return Ok(None);
    };
    let hedone = energy.special && hedone_device_power::owns_skill(catalog, skill_id);
    let amount = if hedone {
        hedone_device_power::available_gain(managers, source_uid, energy.power_id)
            .map_err(CardEnergyError::Hedone)?
            .map_or(0, |(_, amount)| amount)
    } else {
        energy.amount
    };
    let preferred = (hedone && amount > 0)
        || (temporary
            && energy.special
            && pool
                .entity(source_uid)
                .is_some_and(|source| CardMechanic.ultimate_resource_ready(managers, source)))
        || if !energy.special && selected_power_types.is_empty() {
            true
        } else {
            pool.team_type(source_uid).is_some_and(|team| {
                selected_power_types
                    .iter()
                    .any(|(selected_team, selected_power)| {
                        *selected_team == team
                            && (energy.power_id == 0 || *selected_power == energy.power_id)
                    })
            })
        };
    Ok(Some(CardEnergy {
        amount,
        special: energy.special,
        flexible: energy.power_id == 0,
        preferred,
    }))
}
