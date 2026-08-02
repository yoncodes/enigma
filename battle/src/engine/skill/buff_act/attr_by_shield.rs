use crate::engine::{
    entity::attr::AttrId,
    manager::{buff::ActiveBuffFeature, hp::HpManager},
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [raw_attr, rate]
        if AttrId::from_raw(*raw_attr).is_some() && *rate > 0)
}

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId, hp: &HpManager) -> i32 {
    let [_, configured_attr, rate, ..] = feature.values.as_slice() else {
        return 0;
    };
    if AttrId::from_raw(*configured_attr) != Some(attr_id) {
        return 0;
    }
    hp.shield(feature.owner_uid) * rate / 1000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shield_value_scales_its_configured_attribute() {
        let mut hp = HpManager::default();
        hp.set_shield(10, 6000);
        let feature = ActiveBuffFeature {
            owner_uid: 10,
            source_uid: 20,
            buff_uid: 1,
            buff_id: 31170002,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "AttrByShield".into(),
            effect_time: 0,
            effect_condition: 0,
            raw: "955#205#20".into(),
            values: vec![955, 205, 20],
        };

        assert_eq!(attribute_delta(&feature, AttrId::DmgBonus, &hp), 120);
    }

    #[test]
    fn accepts_only_an_attribute_and_positive_rate() {
        use super::super::registry::{BuffActDestination, destination};

        assert!(supports(&[AttrId::DmgBonus.id(), 25]));
        assert!(!supports(&[999, 25]));
        assert!(!supports(&[AttrId::DmgBonus.id(), 0]));
        assert!(!supports(&[AttrId::DmgBonus.id(), 25, 1]));
        assert_eq!(
            destination(955, "AttrByShield", &[AttrId::DmgBonus.id(), 25],),
            Some(BuffActDestination::StateConsumer)
        );
        assert_eq!(
            destination(955, "AttrByShield", &[AttrId::DmgBonus.id(), 0]),
            None
        );
        assert_eq!(
            destination(955, "AttrByHeatScale", &[AttrId::DmgBonus.id(), 25],),
            None
        );
    }
}
