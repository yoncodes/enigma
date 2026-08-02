use crate::engine::{
    entity::attr::AttrId,
    manager::buff::{ActiveBuffFeature, BuffManager},
};

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [tracked_buff_id, 1, raw_attr, per_layer, max_layers]
            if *tracked_buff_id > 0
                && AttrId::from_raw(*raw_attr).is_some()
                && *per_layer != 0
                && *max_layers > 0
    )
}

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId, buffs: &BuffManager) -> i32 {
    let [_, tracked_buff_id, 1, raw_attr, per_layer, max_layers] = feature.values.as_slice() else {
        return 0;
    };
    if AttrId::from_raw(*raw_attr) != Some(attr_id) {
        return 0;
    }
    let tracker_uid = if feature.source_uid != 0 {
        feature.source_uid
    } else {
        feature.owner_uid
    };
    buffs
        .buff_id_or_type_amount(tracker_uid, *tracked_buff_id)
        .min(*max_layers)
        .saturating_mul(*per_layer)
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;

    #[test]
    fn ally_bonus_reads_the_appliers_layers_and_honors_the_cap() {
        crate::test_support::init_config();
        let mut buffs = BuffManager::default();
        buffs.seed(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        buffs: vec![BuffInfo {
                            buff_id: Some(4150001),
                            layer: Some(6),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(20),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        });
        let feature = ActiveBuffFeature {
            owner_uid: 20,
            source_uid: 10,
            buff_uid: 1,
            buff_id: 30940111,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "ModifyAttrByBuffLayer".into(),
            effect_time: 0,
            effect_condition: 0,
            raw: "790#4150001#1#205#10#4".into(),
            values: vec![790, 4150001, 1, 205, 10, 4],
        };

        assert_eq!(attribute_delta(&feature, AttrId::DmgBonus, &buffs), 40);
        assert_eq!(attribute_delta(&feature, AttrId::CriticalDmg, &buffs), 0);
    }
}
