use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute, effect_type_enum::EffectType};

use super::*;
use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
    },
    packet::effect::EffectPacket,
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

#[test]
fn past_shadows_snapshots_source_attack_once_and_projects_both_attributes() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_700),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(300),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(12),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(900),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let command = |source_uid| {
        BuffCommand::Grant(BuffGrant {
            origin,
            source_uid,
            target_uid: 11,
            buff_id: 31450111,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };

    let changes = managers.execute_buff(command(10)).unwrap();
    let added = changes.change.added.as_ref().unwrap();
    assert_eq!(
        added
            .buff
            .act_info
            .iter()
            .find(|info| info.act_id == Some(1152))
            .and_then(|info| info.str_param.as_deref()),
        Some("205#102")
    );
    assert_eq!(
        EffectPacket::buff_changes(&changes.change)
            .iter()
            .map(|effect| effect.effect_type.unwrap_or_default())
            .collect::<Vec<_>>(),
        vec![
            EffectType::Buffactinfoupdate as i32,
            EffectType::Buffadd as i32,
            EffectType::Attr as i32,
            EffectType::Attr as i32,
        ]
    );
    assert_eq!(
        managers.persistent_attribute_delta(11, AttrId::DmgBonus),
        112
    );

    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 12,
            target_uid: 10,
            buff_id: 31460131,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    assert_eq!(managers.buff.fixed_attribute_delta(10, AttrId::Attack), 72);
    let refreshed = managers.execute_buff(command(10)).unwrap();
    assert_eq!(managers.buff.buff_id_amount(11, 31450111), 2);
    assert_eq!(
        managers.persistent_attribute_delta(11, AttrId::DmgBonus),
        122
    );
    let [marker] = refreshed.refresh_initial_state_markers.as_slice() else {
        panic!("expected one source-attribute refresh marker")
    };
    assert_eq!(marker.target_uid, 11);
    assert_eq!(marker.act_id, 1152);
    assert_eq!(marker.str_param.as_deref(), Some("205#106"));
    assert_eq!(
        managers
            .buff
            .snapshot(11, added.buff.uid.unwrap())
            .unwrap()
            .act_info
            .iter()
            .find(|info| info.act_id == Some(1152))
            .and_then(|info| info.str_param.as_deref()),
        Some("205#102")
    );
    assert_ne!(marker.buff_uid, added.buff.uid.unwrap());
    assert_eq!(
        EffectPacket::recorded_buff_changes(&refreshed)
            .iter()
            .map(|effect| effect.effect_type.unwrap_or_default())
            .collect::<Vec<_>>(),
        vec![
            EffectType::Buffactinfoupdate as i32,
            EffectType::Buffupdate as i32,
            EffectType::Buffupdate as i32,
        ]
    );
}
