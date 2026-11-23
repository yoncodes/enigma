use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;

#[test]
fn configured_attributes_scale_by_missing_hp_and_cap() {
    let mut hp = HpManager::default();
    hp.seed(&Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let feature = ActiveBuffFeature {
        owner_uid: 1,
        source_uid: 1,
        buff_uid: 1,
        buff_id: 1,
        amount: 1,
        team_type: 1,
        owner_alive: true,
        act_type: "AttrByLostHp".into(),
        effect_time: 203,
        effect_condition: 0,
        raw: "853#100#205,206#25,25#8".into(),
        values: vec![853, 100, 205, 206, 25, 25, 8],
    };

    assert_eq!(attribute_delta(&feature, AttrId::DmgBonus, &hp), 200);
    assert_eq!(
        attribute_delta(&feature, AttrId::DmgTakenReduction, &hp),
        200
    );
}

#[test]
fn absolute_missing_hp_steps_scale_playmode_attributes() {
    let mut hp = HpManager::default();
    hp.seed(&Fight {
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(195_500_000),
                attr: Some(HeroAttribute {
                    hp: Some(200_000_000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let feature = |raw: &str, act_type: &str| ActiveBuffFeature {
        owner_uid: -1,
        source_uid: -1,
        buff_uid: 1,
        buff_id: 1,
        amount: 1,
        team_type: 2,
        owner_alive: true,
        act_type: act_type.into(),
        effect_time: 203,
        effect_condition: 0,
        raw: raw.into(),
        values: raw
            .split('#')
            .filter_map(|value| value.parse().ok())
            .collect(),
    };

    assert_eq!(
        attribute_delta(
            &feature("853#4000000#215#400#1#1#0", "AttrByLostHp"),
            AttrId::PlaymodeDmgIncrease,
            &hp,
        ),
        400
    );
    assert_eq!(
        attribute_delta(
            &feature("1056#2000000#216#200#1#1#1", "AttrByLostHp"),
            AttrId::PlaymodeDmgImmunity,
            &hp,
        ),
        200
    );
}
