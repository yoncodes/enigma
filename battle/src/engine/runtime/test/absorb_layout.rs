use super::*;

fn opening_round(params: &[&str]) -> FightRound {
    FightRound {
        fight_step: vec![sonettobuf::FightStep {
            act_effect: params
                .iter()
                .map(|param| sonettobuf::ActEffect {
                    fight_step: Some(sonettobuf::FightStep {
                        act_effect: vec![sonettobuf::ActEffect {
                            hurt_info: Some(sonettobuf::FightHurtInfo {
                                absorb_hurt_param: Some((*param).to_owned()),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }],
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn opening_round_detects_two_map_absorb_layout() {
    let round = opening_round(&[r#"{"reduceTeamShareShieldBuffMap":"","reduceShieldBuffMap":""}"#]);

    assert_eq!(
        observed_absorb_hurt_map_layout(&round).unwrap(),
        Some(crate::engine::fight::versions::AbsorbHurtMapLayout::TwoMaps)
    );
}

#[test]
fn opening_round_detects_three_map_absorb_layout() {
    let round = opening_round(&[
        r#"{"consumeFakeHpBuffMap":"","reduceTeamShareShieldBuffMap":"","reduceShieldBuffMap":""}"#,
    ]);

    assert_eq!(
        observed_absorb_hurt_map_layout(&round).unwrap(),
        Some(crate::engine::fight::versions::AbsorbHurtMapLayout::ThreeMaps)
    );
}

#[test]
fn opening_round_without_absorb_evidence_keeps_the_default() {
    assert_eq!(
        observed_absorb_hurt_map_layout(&FightRound::default()).unwrap(),
        None
    );
}

#[test]
fn opening_round_rejects_mixed_absorb_layouts() {
    let round = opening_round(&[
        r#"{"reduceTeamShareShieldBuffMap":"","reduceShieldBuffMap":""}"#,
        r#"{"consumeFakeHpBuffMap":"","reduceTeamShareShieldBuffMap":"","reduceShieldBuffMap":""}"#,
    ]);

    assert!(observed_absorb_hurt_map_layout(&round).is_err());
}

#[test]
fn opening_round_rejects_unsupported_absorb_evidence() {
    for param in [
        "",
        r#"{"reduceTeamShareShieldBuffMap":"","reduceShieldBuffMap":"","other":""}"#,
        r#"{"consumeFakeHpBuffMap":"1#10","reduceTeamShareShieldBuffMap":"","reduceShieldBuffMap":""}"#,
        r#"{"reduceTeamShareShieldBuffMap":0,"reduceShieldBuffMap":""}"#,
    ] {
        assert!(observed_absorb_hurt_map_layout(&opening_round(&[param])).is_err());
    }
}
