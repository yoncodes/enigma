use crate::engine::fight::versions::HurtInfoWireLayout;
use crate::engine::manager::{
    ex_point::{ExPointApplyResult, ExPointKind},
    hp::DamageEffectKind,
};

use super::*;

#[test]
fn clear_universal_card_is_owned_by_the_player_team() {
    assert_eq!(EffectPacket::clear_universal_card().team_type, Some(1));
}

#[test]
fn conduit_skill_group_change_keeps_the_exact_behavior_key() {
    let effect = EffectPacket::conduit_group_selected(10, 1, 3, 60293);

    assert_eq!(effect.target_id, Some(10));
    assert_eq!(
        effect.effect_type,
        Some(EffectType::Deviceskillindex as i32)
    );
    assert_eq!(effect.effect_num, Some(3));
    assert_eq!(effect.config_effect, Some(60293));
}

#[test]
fn hp_change_uses_damage_or_heal_effect_type() {
    let damage = EffectPacket::hp_with_hurt_info_layout(
        HpChange {
            target_uid: 1,
            before: 10,
            delta: -3,
            after: 7,
            max: 10,
            config_effect: 9,
            hurt: None,
            assassinate: false,
            effect_type: 0,
            display_amount: None,
        },
        HurtInfoWireLayout::Version6,
    );
    let heal = EffectPacket::hp_with_hurt_info_layout(
        HpChange {
            target_uid: 1,
            before: 7,
            delta: 2,
            after: 9,
            max: 10,
            config_effect: 8,
            hurt: None,
            assassinate: false,
            effect_type: 0,
            display_amount: None,
        },
        HurtInfoWireLayout::Version6,
    );
    let overheal = EffectPacket::hp_with_hurt_info_layout(
        HpChange {
            target_uid: 1,
            before: 10,
            delta: 0,
            after: 10,
            max: 10,
            config_effect: 0,
            hurt: None,
            assassinate: false,
            effect_type: 0,
            display_amount: Some(2),
        },
        HurtInfoWireLayout::Version6,
    );

    assert_eq!(damage.effect_type, Some(EffectType::Damage as i32));
    assert_eq!(damage.effect_num, Some(3));
    assert_eq!(damage.config_effect, Some(9));
    assert_eq!(heal.effect_type, Some(EffectType::Heal as i32));
    assert_eq!(heal.effect_num, Some(2));
    assert_eq!(overheal.effect_type, Some(EffectType::Heal as i32));
    assert_eq!(overheal.effect_num, Some(2));
}

#[test]
fn crit_is_encoded_by_effect_type() {
    let damage = EffectPacket::hp_with_hurt_info_layout(
        HpChange {
            target_uid: 1,
            before: 10,
            delta: -3,
            after: 7,
            max: 10,
            config_effect: 0,
            hurt: Some(HurtInfoData {
                from_uid: 2,
                is_crit: true,
                career_restraint: false,
                reduce_hp: -3,
                effect_id: 3,
                skill_id: 3,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            }),
            assassinate: false,
            effect_type: 0,
            display_amount: None,
        },
        HurtInfoWireLayout::Version6,
    );

    assert_eq!(damage.effect_type, Some(EffectType::Crit as i32));
    assert_eq!(damage.buff_act_id, Some(0));
    assert_eq!(damage.reserve_id, Some(0));
    assert_eq!(damage.team_type, Some(0));
    assert_eq!(damage.effect_num1, Some(0));
    let hurt = damage.hurt_info.unwrap();
    assert_eq!(hurt.reduce_hp, Some(-3));
    assert_eq!(hurt.critical, None);
    assert_eq!(hurt.effect_id, Some(0));
    assert_eq!(hurt.skill_id, Some(0));
}

#[test]
fn version7_damage_projects_committed_toughness_delta() {
    use crate::engine::manager::toughness::{ToughnessChange, ToughnessState};

    let before = ToughnessState {
        value: 20,
        point: 3,
        segment_value: 100,
        max_point: 3,
        team_type: 2,
        broken: false,
    };
    let after = ToughnessState {
        value: 90,
        point: 2,
        segment_value: 100,
        max_point: 3,
        team_type: 2,
        broken: false,
    };
    let effect = EffectPacket::hp_with_hurt_info_and_toughness_layout(
        HpChange {
            target_uid: -1,
            before: 1_000,
            delta: -30,
            after: 970,
            max: 1_000,
            config_effect: 0,
            hurt: Some(HurtInfoData {
                from_uid: 1,
                is_crit: false,
                career_restraint: true,
                reduce_hp: -30,
                effect_id: 0,
                skill_id: 1,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            }),
            assassinate: false,
            effect_type: 0,
            display_amount: None,
        },
        Some(ToughnessChange {
            target_uid: -1,
            before,
            value_delta: -70,
            point_delta: 1,
            after,
            broke: false,
        }),
        HurtInfoWireLayout::Version7,
    );

    let hurt = effect.hurt_info.unwrap();
    assert_eq!(hurt.toughness_value, Some(-70));
    assert_eq!(hurt.toughness_point, Some(1));
    assert_eq!(hurt.broken, Some(false));
}

#[test]
fn toughness_recovery_uses_the_captured_point_and_segment_payload() {
    let effect =
        EffectPacket::toughness_recover(crate::engine::manager::toughness::ToughnessRecovery {
            target_uid: -1,
            point: 3,
            value: 60_900,
            team_type: 2,
        });

    assert_eq!(effect.target_id, Some(-1));
    assert_eq!(
        effect.effect_type,
        Some(EffectType::Toughnessrecover as i32)
    );
    assert_eq!(effect.reserve_str.as_deref(), Some("3,60900"));
    assert_eq!(effect.team_type, Some(2));
}

#[test]
fn fully_absorbed_buff_damage_keeps_its_exact_buff_act_opcode() {
    let effect = EffectPacket::fully_absorbed_damage_with_hurt_info_layout(
        20,
        DamageRecord {
            amount: 400,
            config_effect: 0,
            effect_kind: DamageEffectKind::Genesis,
            assassinate: false,
            hurt: HurtInfoData {
                from_uid: 10,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::Buff,
                buff_act_id: 721,
                buff_uid: 82,
                hurt_effect_type: EffectType::Origindamage as i32,
                display_amount: Some(400),
            },
        },
        HurtInfoWireLayout::Version6,
    );

    assert_eq!(effect.effect_num, Some(0));
    assert_eq!(effect.buff_act_id, Some(721));
    assert_eq!(effect.hurt_info.as_ref().unwrap().buff_act_id, Some(721));
}

#[test]
fn assassinate_is_carried_by_the_damage_change() {
    let effect = EffectPacket::hp_with_hurt_info_layout(
        HpChange {
            target_uid: -1,
            before: 1_000,
            delta: -500,
            after: 500,
            max: 1_000,
            config_effect: -1,
            hurt: Some(HurtInfoData {
                from_uid: 10,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 31220131,
                skill_id: 31220131,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            }),
            assassinate: true,
            effect_type: 0,
            display_amount: Some(500),
        },
        HurtInfoWireLayout::Version6,
    );

    assert_eq!(effect.hurt_info.unwrap().assassinate, Some(true));
}

#[test]
fn direct_power_change_uses_delta_without_a_power_snapshot() {
    let effect = EffectPacket::eureka(EurekaApplyResult {
        source_uid: 1,
        target_uid: 1,
        power_id: 1,
        before: 0,
        requested_delta: 4,
        applied_delta: 4,
        after: 4,
        overflow: 0,
        max: 8,
        effect_type: EffectType::Powerchange as i32,
    });

    assert_eq!(effect.effect_num, Some(4));
    assert_eq!(effect.config_effect, Some(1));
    assert!(effect.power_info.is_none());
}

#[test]
fn ordinary_ex_point_spend_is_a_change_not_a_buff_act_delete() {
    let change = ExPointApplyResult {
        source_uid: 1,
        target_uid: 1,
        kind: ExPointKind::Common,
        before: 5,
        requested_delta: -5,
        applied_delta: -5,
        after: 0,
        overflow: 0,
        cap: 5,
        effect_type: 0,
        config_effect: 0,
    };

    assert_eq!(
        ex_point_effect_type(change),
        EffectType::Expointchange as i32
    );
}

#[test]
fn buff_update_and_delete_use_snapshot_without_scalar_buff_id() {
    use crate::engine::manager::buff::{BuffRemoveResult, BuffUpdateResult};
    use sonettobuf::BuffInfo;

    let before = BuffInfo {
        uid: Some(2),
        buff_id: Some(101),
        layer: Some(1),
        ..Default::default()
    };
    let after = BuffInfo {
        layer: Some(3),
        ..before.clone()
    };

    let update = [EffectPacket::buff_update(&BuffUpdateResult {
        target_uid: 10,
        before,
        after: after.clone(),
    })];
    let delete = [EffectPacket::buff_delete(&BuffRemoveResult {
        target_uid: 10,
        before_amount: 1,
        buff: after,
        config_effect: 0,
        delete_reason: None,
        depleted: false,
    })];

    assert_eq!(update[0].effect_type, Some(EffectType::Buffupdate as i32));
    assert_eq!(update[0].effect_num, Some(0));
    assert_eq!(update[0].buff.as_ref().unwrap().buff_id, Some(101));
    assert_eq!(delete[0].effect_type, Some(EffectType::Buffdel as i32));
    assert_eq!(delete[0].effect_num1, Some(0));
    assert_eq!(delete[0].buff.as_ref().unwrap().duration, Some(0));

    let depleted = EffectPacket::buff_delete(&BuffRemoveResult {
        target_uid: 10,
        before_amount: 3,
        buff: BuffInfo {
            duration: Some(1),
            ..Default::default()
        },
        config_effect: 0,
        delete_reason: None,
        depleted: true,
    });
    assert_eq!(depleted.buff.unwrap().duration, Some(1));
}

#[test]
fn duration_only_refresh_does_not_emit_an_attribute_marker() {
    crate::test_support::init_config();
    let before = BuffInfo {
        uid: Some(2),
        buff_id: Some(530000112),
        duration: Some(2),
        ..Default::default()
    };
    let after = BuffInfo {
        duration: Some(1),
        ..before.clone()
    };
    let effects = EffectPacket::buff_changes(&crate::engine::manager::buff::BuffReplaceResult {
        removed: Vec::new(),
        added: None,
        refreshed: vec![BuffUpdateResult {
            target_uid: 10,
            before,
            after,
        }],
        rejected: None,
        fanout: Vec::new(),
    });

    assert_eq!(
        effects
            .iter()
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![Some(EffectType::Buffupdate as i32)]
    );
}

#[test]
fn stack_consumption_does_not_emit_an_attribute_marker() {
    crate::test_support::init_config();
    let before = BuffInfo {
        uid: Some(2),
        buff_id: Some(530000111),
        layer: Some(3),
        ..Default::default()
    };
    let after = BuffInfo {
        layer: Some(2),
        ..before.clone()
    };
    let effects = EffectPacket::buff_changes(&crate::engine::manager::buff::BuffReplaceResult {
        removed: Vec::new(),
        added: None,
        refreshed: vec![BuffUpdateResult {
            target_uid: 10,
            before,
            after,
        }],
        rejected: None,
        fanout: Vec::new(),
    });

    assert_eq!(
        effects
            .iter()
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![Some(EffectType::Buffupdate as i32)]
    );
}

#[test]
fn stack_refresh_emits_configured_refresh_markers() {
    crate::test_support::init_config();
    let before = BuffInfo {
        uid: Some(2),
        buff_id: Some(31130122),
        layer: Some(1),

        ..Default::default()
    };
    let after = BuffInfo {
        layer: Some(2),
        ..before.clone()
    };
    let effects = EffectPacket::buff_changes(&crate::engine::manager::buff::BuffReplaceResult {
        removed: Vec::new(),
        added: None,
        refreshed: vec![BuffUpdateResult {
            target_uid: 10,
            before,
            after,
        }],
        rejected: None,
        fanout: Vec::new(),
    });

    assert_eq!(
        effects
            .iter()
            .map(|effect| effect.effect_type)
            .collect::<Vec<_>>(),
        vec![
            Some(EffectType::Buffupdate as i32),
            Some(EffectType::None as i32)
        ]
    );
}

#[test]
fn scene_change_updates_the_scene_parameter_before_switching() {
    let [parameter, change] = EffectPacket::scene_change(14501);

    assert_eq!(
        parameter.effect_type,
        Some(EffectType::Fightparamchange as i32)
    );
    assert_eq!(parameter.reserve_str.as_deref(), Some("16#14501"));
    assert_eq!(change.effect_type, Some(EffectType::Changescene as i32));
    assert_eq!(change.effect_num, Some(14501));
}
