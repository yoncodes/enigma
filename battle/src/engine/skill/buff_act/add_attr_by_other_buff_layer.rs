use crate::engine::{
    entity::attr::AttrId,
    manager::buff::{ActiveBuffFeature, BuffManager},
};

#[derive(Clone, Copy)]
pub enum LayerScope {
    Source,
    SourceOrOwner,
}

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [primary_attr, base, other_buff_id, per_layer, layer_limit, capped_attr, _]
            if AttrId::from_raw(*primary_attr).is_some()
                && *base != 0
                && *other_buff_id > 0
                && *per_layer != 0
                && *layer_limit > 0
                && AttrId::from_raw(*capped_attr).is_some()
    )
}

pub fn attribute_delta(
    feature: &ActiveBuffFeature,
    attr_id: AttrId,
    buffs: &BuffManager,
    scope: LayerScope,
) -> i32 {
    let [
        _,
        primary_attr,
        base,
        other_buff_id,
        per_layer,
        layer_limit,
        rest @ ..,
    ] = feature.values.as_slice()
    else {
        return 0;
    };
    let source_layer = buffs.max_id_or_type_layer(feature.source_uid, *other_buff_id);
    let layer = match scope {
        LayerScope::Source => source_layer,
        LayerScope::SourceOrOwner => {
            source_layer.max(buffs.max_id_or_type_layer(feature.owner_uid, *other_buff_id))
        }
    }
    .min((*layer_limit).max(0));
    let primary = (*primary_attr == attr_id as i32).then_some(base + per_layer * layer);
    let capped = match rest {
        [capped_attr, capped_value, ..]
            if layer >= *layer_limit && *capped_attr == attr_id as i32 =>
        {
            Some(*capped_value)
        }
        _ => None,
    };
    primary.unwrap_or_default() + capped.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::manager::BattleManagers;

    #[test]
    fn fear_of_death_reads_charons_consternation_layers() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    buffs: vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(31280114),
                        layer: Some(4),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    buffs: vec![BuffInfo {
                        uid: Some(-1),
                        buff_id: Some(31280111),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let feature = managers
            .buff
            .active_features(&managers.hp)
            .into_iter()
            .find(|feature| feature.act_id() == Some(1036))
            .unwrap();

        assert_eq!(
            attribute_delta(
                &feature,
                AttrId::CriticalDef,
                &managers.buff,
                LayerScope::SourceOrOwner,
            ),
            -400
        );
        assert_eq!(
            attribute_delta(
                &feature,
                AttrId::DmgTakenReduction,
                &managers.buff,
                LayerScope::SourceOrOwner,
            ),
            -300
        );
    }

    fn rhiannon_managers(
        rapport_buff_id: i32,
        rapport_layer: i32,
        debuff_id: i32,
        target_rapport_layer: Option<i32>,
    ) -> BattleManagers {
        let mut target_buffs = vec![BuffInfo {
            uid: Some(-1),
            buff_id: Some(debuff_id),
            from_uid: Some(10),
            ..Default::default()
        }];
        if let Some(layer) = target_rapport_layer {
            target_buffs.push(BuffInfo {
                uid: Some(-2),
                buff_id: Some(rapport_buff_id),
                layer: Some(layer),
                from_uid: Some(-1),
                ..Default::default()
            });
        }
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    buffs: vec![BuffInfo {
                        uid: Some(1),
                        buff_id: Some(rapport_buff_id),
                        layer: Some(rapport_layer),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    buffs: target_buffs,
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        BattleManagers::seeded(&fight)
    }

    #[test]
    fn rhiannon_critical_defense_reads_source_rapport_and_caps_layers() {
        crate::test_support::init_config();

        for (rapport_layer, expected) in [(0, -150), (3, -240), (8, -300)] {
            let managers = rhiannon_managers(31460003, rapport_layer, 31460212, None);
            let feature = managers
                .buff
                .active_features(&managers.hp)
                .into_iter()
                .find(|feature| feature.act_id() == Some(1141))
                .unwrap();

            assert_eq!(
                attribute_delta(
                    &feature,
                    AttrId::CriticalDef,
                    &managers.buff,
                    LayerScope::Source,
                ),
                expected
            );
            assert_eq!(
                managers.persistent_attribute_delta(-1, AttrId::CriticalDef),
                expected
            );
        }

        let managers = rhiannon_managers(31460004, 8, 31460214, None);
        assert_eq!(
            managers.persistent_attribute_delta(-1, AttrId::CriticalDef),
            -410
        );

        let managers = rhiannon_managers(31460003, 3, 31460212, Some(8));
        assert_eq!(
            managers.persistent_attribute_delta(-1, AttrId::CriticalDef),
            -240
        );
    }
}
