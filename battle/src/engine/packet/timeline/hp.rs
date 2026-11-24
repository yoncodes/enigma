use sonettobuf::ActEffect;

use crate::engine::{
    fight::versions::{AbsorbHurtMapLayout, HurtInfoWireLayout},
    manager::{hp::HpChanges, shield::ShieldChanges},
    packet::effect::EffectPacket,
};

pub(super) fn project_hp(
    changes: &HpChanges,
    writes_reduce_hp: bool,
    hurt_info_layout: HurtInfoWireLayout,
    absorb_map_layout: AbsorbHurtMapLayout,
) -> Vec<ActEffect> {
    if let Some(config_effect) = changes.kill {
        return vec![EffectPacket::kill(changes.target_uid, config_effect)];
    }
    let mut effects = Vec::new();
    // A shared hit lands after its stack update and every ally's share.
    if let Some(shared) = &changes.shared_hurt {
        effects.extend(EffectPacket::recorded_buff_changes(&shared.consumed));
        for share in &shared.shares {
            effects.extend(project_hp(
                share,
                writes_reduce_hp,
                hurt_info_layout,
                absorb_map_layout,
            ));
        }
    }
    if let Some(shield) = changes
        .shield_absorbed
        .filter(|_| hurt_info_layout == HurtInfoWireLayout::Version6)
    {
        effects.push(EffectPacket::shield(
            shield.target_uid,
            shield.after,
            changes
                .damage
                .map(|damage| damage.hurt.hurt_effect_type)
                .unwrap_or_default(),
        ));
    }
    if let Some(shield) = changes.shield_granted {
        effects.push(EffectPacket::shield_value_change(
            shield.target_uid,
            shield.added,
        ));
    }
    if let Some(hp) = changes.hp {
        let hp = apply_hp_wire_layout(hp, writes_reduce_hp);
        let mut effect = hp
            .hurt
            .filter(|hurt| hurt.damage_from == crate::engine::manager::hp::HurtDamageFromType::Buff)
            .map(|hurt| {
                EffectPacket::damage_by_buff_act_with_hurt_info_layout(
                    hp,
                    hurt.buff_act_id,
                    hurt_info_layout,
                    absorb_map_layout,
                )
            })
            .unwrap_or_else(|| {
                EffectPacket::hp_with_hurt_info_and_toughness_layout(
                    hp,
                    changes.toughness,
                    hurt_info_layout,
                    absorb_map_layout,
                )
            });
        apply_absorbed_shield_wire(
            &mut effect,
            changes.fake_hp_consumed,
            changes.team_shared_shield_absorbed,
            changes.shield_absorbed,
            hurt_info_layout,
            absorb_map_layout,
        );
        effects.push(effect);
    } else if let Some(damage) = changes.damage {
        let mut effect = EffectPacket::fully_absorbed_damage_with_toughness_layout(
            changes.target_uid,
            damage,
            changes.toughness,
            hurt_info_layout,
            absorb_map_layout,
        );
        apply_absorbed_shield_wire(
            &mut effect,
            changes.fake_hp_consumed,
            changes.team_shared_shield_absorbed,
            changes.shield_absorbed,
            hurt_info_layout,
            absorb_map_layout,
        );
        effects.push(effect);
    }
    if let Some(removed) = &changes.team_shared_shield_removed {
        effects.extend(EffectPacket::recorded_buff_changes(removed));
    }
    if let Some(removed) = &changes.shield_broken {
        effects.push(EffectPacket::shield_delete(
            removed.absorption.target_uid,
            removed.absorption.after,
        ));
        effects.extend(EffectPacket::recorded_buff_changes(&removed.carrier));
    }
    if let Some(max_hp) = changes.max_hp {
        let wire = (changes.origin.domain == crate::engine::skill::rule::RuleDomain::BuffAct)
            .then(|| {
                crate::engine::skill::buff_act::wire::find(
                    changes.origin.key.opcode,
                    changes.origin.key.type_name,
                )
                .and_then(|definition| definition.max_hp)
            })
            .flatten();
        let plan = wire.map_or(
            crate::engine::skill::buff_act::wire::MaxHpWirePlan {
                max_hp_repeats: 1,
                current_hp_repeats: 1,
                buff_act_id: 0,
            },
            |rule| {
                max_hp
                    .replaced_removal
                    .then(|| rule.replaced_removal_plan())
                    .flatten()
                    .unwrap_or_else(|| {
                        rule.plan(
                            changes.source_uid,
                            changes.target_uid,
                            max_hp.before_current,
                            max_hp.after_current,
                            max_hp.delta,
                        )
                    })
            },
        );
        for index in 0..plan.max_hp_repeats {
            effects.push(EffectPacket::max_hp_change(
                max_hp.target_uid,
                max_hp.after_max,
                plan.buff_act_id,
            ));
            if index < plan.current_hp_repeats {
                effects.push(EffectPacket::current_hp_change(
                    max_hp.target_uid,
                    max_hp.after_current,
                ));
            }
        }
    }
    if let Some(death) = changes.death {
        effects.push(EffectPacket::dead(death.transition().target_uid));
    }
    effects
}

pub(super) fn project_shield(changes: &ShieldChanges) -> Vec<ActEffect> {
    let shield = changes.hp.as_ref().and_then(|hp| hp.shield_granted);
    if let Some(buff) = &changes.buff {
        let mut effects = EffectPacket::recorded_buff_changes(buff);
        if let Some(shield) = shield
            && let Some(marker) = effects.iter_mut().find(|effect| {
                effect.effect_type == Some(sonettobuf::effect_type_enum::EffectType::Shield as i32)
            })
        {
            marker.effect_num = Some(shield.after);
        } else if let Some(shield) = shield {
            effects.insert(
                0,
                EffectPacket::shield_value_change(shield.target_uid, shield.added),
            );
        }
        effects
    } else if let Some(shield) = changes.team_shared {
        vec![EffectPacket::buff_act_info(
            shield.owner_uid,
            shield.buff_uid,
            shield.buff_act_id,
            vec![shield.after],
        )]
    } else {
        shield
            .map(|shield| EffectPacket::shield_value_change(shield.target_uid, shield.added))
            .into_iter()
            .collect()
    }
}

fn apply_absorbed_shield_wire(
    effect: &mut ActEffect,
    fake_hp: Option<crate::engine::manager::hp::FakeHpConsumption>,
    team_shared: Option<crate::engine::manager::hp::TeamSharedShieldAbsorption>,
    shield: Option<crate::engine::manager::hp::ShieldChange>,
    layout: HurtInfoWireLayout,
    absorb_map_layout: AbsorbHurtMapLayout,
) {
    if layout != HurtInfoWireLayout::Version7
        || (fake_hp.is_none() && team_shared.is_none() && shield.is_none())
    {
        return;
    }
    let Some(hurt) = &mut effect.hurt_info else {
        return;
    };
    let team_shared = team_shared
        .map(|change| format!("{}#{}", change.buff_uid, change.consumed))
        .unwrap_or_default();
    let fake_hp = fake_hp
        .map(|change| format!("{}#{}", change.buff_uid, change.consumed))
        .unwrap_or_default();
    let shield = shield
        .map(|change| format!("{}#{}", change.buff_uid, change.absorbed))
        .unwrap_or_default();
    hurt.absorb_hurt_param = Some(match absorb_map_layout {
        AbsorbHurtMapLayout::TwoMaps => format!(
            r#"{{"reduceTeamShareShieldBuffMap":"{team_shared}","reduceShieldBuffMap":"{shield}"}}"#
        ),
        AbsorbHurtMapLayout::ThreeMaps => format!(
            r#"{{"consumeFakeHpBuffMap":"{fake_hp}","reduceTeamShareShieldBuffMap":"{team_shared}","reduceShieldBuffMap":"{shield}"}}"#
        ),
    });
}

fn apply_hp_wire_layout(
    mut change: crate::engine::manager::hp::HpChange,
    writes_reduce_hp: bool,
) -> crate::engine::manager::hp::HpChange {
    if !writes_reduce_hp && let Some(hurt) = &mut change.hurt {
        hurt.reduce_hp = 0;
    }
    change
}
