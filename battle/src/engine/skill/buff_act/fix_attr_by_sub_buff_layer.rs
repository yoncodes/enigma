use crate::engine::{
    entity::attr::AttrId,
    manager::buff::{ActiveBuffFeature, BuffManager},
};

pub fn supports(args: &[i32]) -> bool {
    let [required_buff, attributes @ ..] = args else {
        return false;
    };
    *required_buff > 0
        && !attributes.is_empty()
        && attributes.len().is_multiple_of(3)
        && attributes
            .chunks_exact(3)
            .all(|values| AttrId::from_raw(values[0]).is_some())
}

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId, buffs: &BuffManager) -> i32 {
    let mut fields = feature.raw.split('#');
    let (Some(_), Some(required_buff)) = (fields.next(), fields.next()) else {
        return 0;
    };
    let Ok(required_buff) = required_buff.parse() else {
        return 0;
    };
    let attributes = fields
        .flat_map(|field| field.split(','))
        .filter_map(|value| value.parse().ok())
        .collect::<Vec<_>>();
    let layer = buffs.buff_id_or_type_amount(feature.owner_uid, required_buff);
    layer_attribute_delta(&attributes, layer, attr_id)
}

fn layer_attribute_delta(attributes: &[i32], layer: i32, attr_id: AttrId) -> i32 {
    if layer <= 0 {
        return 0;
    }
    attributes
        .chunks_exact(3)
        .filter_map(|values| {
            let [configured_attr, base, per_extra] = values else {
                return None;
            };
            (AttrId::from_raw(*configured_attr) == Some(attr_id))
                .then_some(*base + *per_extra * (layer - 1))
        })
        .sum()
}

pub fn owner_attribute_delta(buffs: &BuffManager, owner_uid: i64, attr_id: AttrId) -> i32 {
    let Some(catalog) = buffs
        .try_catalog()
        .or_else(crate::catalog::BattleCatalog::try_global)
    else {
        return 0;
    };
    let mut total = 0;
    for buff in buffs.active_for(owner_uid) {
        let Some(buff_id) = buff.buff_id else {
            continue;
        };
        for raw in catalog.buff_feature_rows(buff_id) {
            let values = raw
                .split('#')
                .flat_map(|value| value.split(','))
                .filter_map(|value| value.parse::<i32>().ok())
                .collect::<Vec<_>>();
            let [act_id, required_buff, attributes @ ..] = values.as_slice() else {
                continue;
            };
            if catalog.buff_act_definition(*act_id).map(|act| act.kind)
                == Some(super::registry::BuffActKind::FixAttrBySubBuffLayer)
            {
                total += layer_attribute_delta(
                    attributes,
                    buffs.buff_id_or_type_amount(owner_uid, *required_buff),
                    attr_id,
                );
            }
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;

    #[test]
    fn keeps_its_configured_attack_attribute() {
        assert_eq!(
            layer_attribute_delta(&[201, 300, 0], 2, AttrId::CriticalRate),
            300
        );
    }

    #[test]
    fn applies_each_attribute_from_the_tracked_layer_count() {
        assert_eq!(
            layer_attribute_delta(&[104, -100, -20, 206, -100, -20], 3, AttrId::MentalDef),
            -140
        );
        assert_eq!(
            layer_attribute_delta(
                &[104, -100, -20, 206, -100, -20],
                3,
                AttrId::DmgTakenReduction,
            ),
            -140
        );
    }

    #[test]
    fn validates_a_tracked_buff_and_attribute_triples() {
        assert!(supports(&[31260151, 201, 300, 0]));
        assert!(supports(&[31130122, 104, -100, -20, 206, -100, -20]));
        assert!(!supports(&[0, 201, 600, 0]));
        assert!(!supports(&[31260151]));
        assert!(!supports(&[31260151, 999, 300, 0]));
        assert!(!supports(&[31260151, 201, 300]));
    }

    #[test]
    fn owner_delta_uses_owned_catalog_with_global_compatibility() {
        crate::test_support::init_config();
        let mut buffs = BuffManager::default();
        buffs.seed(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    buffs: vec![
                        BuffInfo {
                            buff_id: Some(31130122),
                            layer: Some(3),
                            ..Default::default()
                        },
                        BuffInfo {
                            buff_id: Some(31130124),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });

        assert_eq!(owner_attribute_delta(&buffs, 1, AttrId::MentalDef), -140);
        buffs.set_catalog(crate::catalog::BattleCatalog::new(
            crate::test_support::game_data(),
        ));
        assert_eq!(owner_attribute_delta(&buffs, 1, AttrId::MentalDef), -140);
    }
}
