use super::*;

fn feature(values: Vec<i32>) -> ActiveBuffFeature {
    ActiveBuffFeature {
        owner_uid: 1,
        source_uid: 1,
        buff_uid: 1,
        buff_id: 1,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "AttrByHeatScale".to_owned(),
        effect_time: 0,
        effect_condition: 0,
        raw: String::new(),
        values,
    }
}

#[test]
fn explicit_step_snapshots_the_scaled_attribute_value() {
    let critical_rate = feature(vec![1053, AttrId::CriticalRate.id(), 5, 1_000_000, 10_000]);
    assert_eq!(snapshot_delta(&critical_rate, 37_791), Some(18));
}

#[test]
fn snapshot_scales_fractional_glow_before_truncating() {
    let damage_bonus = feature(vec![1053, AttrId::DmgBonus.id(), 150, 600_000, 100_000]);
    let incantation_might = feature(vec![
        1053,
        AttrId::IncantationMight.id(),
        60,
        600_000,
        100_000,
    ]);
    let upgraded_might = feature(vec![
        1053,
        AttrId::IncantationMight.id(),
        80,
        600_000,
        100_000,
    ]);
    assert_eq!(snapshot_delta(&damage_bonus, 37_791), Some(56));
    assert_eq!(snapshot_delta(&incantation_might, 21_600), Some(12));
    assert_eq!(snapshot_delta(&upgraded_might, 37_000), Some(29));
    assert_eq!(snapshot_delta(&upgraded_might, 37_791), Some(30));
}

#[test]
fn configured_heat_scale_buff_snapshots_the_calculated_attribute() {
    crate::test_support::init_config();

    let infos = snapshot(31390175, 21_600).unwrap();

    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].act_id, Some(1053));
    assert_eq!(infos[0].param, vec![32]);
}
