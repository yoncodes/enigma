use anyhow::{Context, Result};

pub(crate) fn current() -> Result<i32> {
    config::configs::get()
        .r#const
        .get(1707) // ConstEnum.FightVersion in Lua.
        .context("FightVersion config 1707 is missing")?
        .value
        .parse()
        .context("FightVersion config 1707 is not an integer")
}

pub(crate) fn writes_reduce_hp(version: i32) -> bool {
    version == 7
}

pub(crate) fn writes_change_round_number(version: i32) -> bool {
    version == 7
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HurtInfoWireLayout {
    Version6,
    Version7,
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

#[cfg(test)]
mod tests {
    use super::{
        HurtInfoWireLayout, RedealWireLayout, RoundStartSetupLayout, hurt_info_wire_layout,
        redeal_wire_layout, round_start_setup_layout, writes_change_round_number, writes_reduce_hp,
    };

    #[test]
    fn reduce_hp_wire_field_is_confirmed_only_for_version_seven() {
        assert!(!writes_reduce_hp(6));
        assert!(writes_reduce_hp(7));
        assert!(!writes_change_round_number(6));
        assert!(writes_change_round_number(7));
    }

    #[test]
    fn hurt_info_wire_layout_is_selected_by_fight_version() {
        assert_eq!(hurt_info_wire_layout(6), Some(HurtInfoWireLayout::Version6));
        assert_eq!(hurt_info_wire_layout(7), Some(HurtInfoWireLayout::Version7));
        assert_eq!(hurt_info_wire_layout(8), None);
    }

    #[test]
    fn round_start_setup_layout_is_selected_by_fight_version() {
        assert_eq!(
            round_start_setup_layout(6),
            Some(RoundStartSetupLayout::Version6)
        );
        assert_eq!(
            round_start_setup_layout(7),
            Some(RoundStartSetupLayout::Version7)
        );
        assert_eq!(round_start_setup_layout(8), None);
    }

    #[test]
    fn redeal_wire_layout_is_selected_by_fight_version() {
        assert_eq!(redeal_wire_layout(6), Some(RedealWireLayout::Version6));
        assert_eq!(redeal_wire_layout(7), Some(RedealWireLayout::Version7));
        assert_eq!(redeal_wire_layout(8), None);
    }
}
