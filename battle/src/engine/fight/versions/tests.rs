use super::{
    AbsorbHurtMapLayout, ChangeRoundWireLayout, HurtInfoWireLayout, RedealWireLayout,
    RoundStartSetupLayout, absorb_hurt_map_layout, change_round_wire_layout, hurt_info_wire_layout,
    redeal_wire_layout, round_start_setup_layout, writes_reduce_hp,
};

#[test]
fn reduce_hp_wire_field_is_confirmed_only_for_version_seven() {
    assert!(!writes_reduce_hp(6));
    assert!(writes_reduce_hp(7));
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

#[test]
fn change_round_wire_layout_is_selected_by_fight_version() {
    assert_eq!(
        change_round_wire_layout(6),
        Some(ChangeRoundWireLayout::Incremental)
    );
    assert_eq!(
        change_round_wire_layout(7),
        Some(ChangeRoundWireLayout::CurrentRound)
    );
    assert_eq!(change_round_wire_layout(8), None);
}

#[test]
fn absorb_hurt_map_layout_follows_the_fight_version() {
    assert_eq!(absorb_hurt_map_layout(6), AbsorbHurtMapLayout::TwoMaps);
    assert_eq!(absorb_hurt_map_layout(7), AbsorbHurtMapLayout::ThreeMaps);
}
