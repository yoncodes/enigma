use crate::engine::{
    entity::attr::AttrId,
    manager::buff::{ActiveBuffFeature, BuffManager},
};

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

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId, buffs: &BuffManager) -> i32 {
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
    let layer = source_layer
        .max(buffs.max_id_or_type_layer(feature.owner_uid, *other_buff_id))
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
            attribute_delta(&feature, AttrId::CriticalDef, &managers.buff),
            -400
        );
        assert_eq!(
            attribute_delta(&feature, AttrId::DmgTakenReduction, &managers.buff),
            -300
        );
    }
}
