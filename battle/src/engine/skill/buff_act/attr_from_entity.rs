use crate::engine::{
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        attribute::AttributeManager,
        buff::{ActiveBuffFeature, BuffManager},
        hp::HpManager,
    },
};

fn delta_from_values(
    values: &[i32],
    act_type: &str,
    owner_uid: i64,
    attributes: &AttributeManager,
    buffs: &BuffManager,
    hp: &HpManager,
) -> Option<(AttrId, i32)> {
    let [act_id, source_attr, target_attr, rate] = values else {
        return None;
    };
    if super::registry::kind(*act_id, act_type)
        != Some(super::registry::BuffActKind::AttrFromEntity)
    {
        return None;
    }
    let source_attr = AttrId::from_raw(*source_attr)?;
    let target_attr = AttrId::from_raw(*target_attr)?;
    let source = match source_attr {
        AttrId::Hp => hp.base_max(owner_uid),
        AttrId::CurrentHp => hp.current(owner_uid),
        AttrId::Attack | AttrId::RealityDef | AttrId::MentalDef | AttrId::CriticalTechnique => {
            let rate = 1000
                + attributes.get(owner_uid, source_attr)
                + buffs.attribute_delta(owner_uid, source_attr);
            attributes.base(owner_uid, source_attr) * rate.max(0) / 1000
        }
        _ => attributes.get(owner_uid, source_attr) + buffs.attribute_delta(owner_uid, source_attr),
    };
    Some((target_attr, source.max(0) * rate / 1000))
}

pub fn configured_delta(
    buff_id: i32,
    owner_uid: i64,
    managers: &BattleManagers,
) -> Option<(AttrId, i32)> {
    let catalog = managers.catalog();
    catalog.buff_feature_rows(buff_id).iter().find_map(|raw| {
        let values = raw
            .split('#')
            .filter_map(|value| value.parse::<i32>().ok())
            .collect::<Vec<_>>();
        let act_id = *values.first()?;
        let act_type = catalog.buff_act_definition(act_id)?.key.type_name;
        delta_from_values(
            &values,
            act_type,
            owner_uid,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
        )
    })
}

pub fn active_delta(managers: &BattleManagers, owner_uid: i64, attr_id: AttrId) -> i32 {
    active_feature_delta(
        &managers.buff.active_features(&managers.hp),
        owner_uid,
        attr_id,
        &managers.attribute,
        &managers.buff,
        &managers.hp,
    )
}

pub fn active_feature_delta(
    features: &[ActiveBuffFeature],
    owner_uid: i64,
    attr_id: AttrId,
    attributes: &AttributeManager,
    buffs: &BuffManager,
    hp: &HpManager,
) -> i32 {
    features
        .iter()
        .filter(|feature| feature.owner_uid == owner_uid)
        .filter_map(|feature| {
            delta_from_values(
                &feature.values,
                &feature.act_type,
                owner_uid,
                attributes,
                buffs,
                hp,
            )
        })
        .filter_map(|(actual, delta)| (actual == attr_id).then_some(delta))
        .sum()
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;

    #[test]
    fn opcode_820_derives_damage_bonus_from_the_entity_hp_attribute() {
        crate::test_support::init_config();
        let mut managers = BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(13_366),
                    attr: Some(HeroAttribute {
                        hp: Some(13_366),
                        attack: Some(1_848),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(2),
                        buff_id: Some(30091119),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        managers.hp.add_max_snapshot(1, 3_554);

        assert_eq!(
            configured_delta(31260211, 1, &managers),
            Some((AttrId::DmgBonus, 334))
        );
        assert_eq!(active_delta(&managers, 1, AttrId::PoisonDmgBonus), 369);
    }
}
