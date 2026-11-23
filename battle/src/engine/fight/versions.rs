use anyhow::Result;

pub(crate) fn current(catalog: crate::catalog::BattleCatalog) -> Result<i32> {
    match catalog.fight_version() {
        crate::catalog::ConfiguredFightVersion::Missing => {
            Err(anyhow::anyhow!("FightVersion config 1707 is missing"))
        }
        crate::catalog::ConfiguredFightVersion::Invalid => Err(anyhow::anyhow!(
            "FightVersion config 1707 is not an integer"
        )),
        crate::catalog::ConfiguredFightVersion::Value(version) => Ok(version),
    }
}

pub(crate) fn writes_reduce_hp(version: i32) -> bool {
    version == 7
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HurtInfoWireLayout {
    Version6,
    Version7,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AbsorbHurtMapLayout {
    TwoMaps,
    #[default]
    ThreeMaps,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RoundStartSetupLayout {
    Version6,
    Version7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RedealWireLayout {
    Version6,
    Version7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChangeRoundWireLayout {
    Incremental,
    CurrentRound,
}

pub(crate) const fn absorb_hurt_map_layout(version: i32) -> AbsorbHurtMapLayout {
    if version == 6 {
        AbsorbHurtMapLayout::TwoMaps
    } else {
        AbsorbHurtMapLayout::ThreeMaps
    }
}

pub(crate) const fn hurt_info_wire_layout(version: i32) -> Option<HurtInfoWireLayout> {
    match version {
        6 => Some(HurtInfoWireLayout::Version6),
        7 => Some(HurtInfoWireLayout::Version7),
        _ => None,
    }
}

pub(crate) const fn round_start_setup_layout(version: i32) -> Option<RoundStartSetupLayout> {
    match version {
        6 => Some(RoundStartSetupLayout::Version6),
        7 => Some(RoundStartSetupLayout::Version7),
        _ => None,
    }
}

pub(crate) const fn redeal_wire_layout(version: i32) -> Option<RedealWireLayout> {
    match version {
        6 => Some(RedealWireLayout::Version6),
        7 => Some(RedealWireLayout::Version7),
        _ => None,
    }
}

pub(crate) const fn change_round_wire_layout(version: i32) -> Option<ChangeRoundWireLayout> {
    match version {
        6 => Some(ChangeRoundWireLayout::Incremental),
        7 => Some(ChangeRoundWireLayout::CurrentRound),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
