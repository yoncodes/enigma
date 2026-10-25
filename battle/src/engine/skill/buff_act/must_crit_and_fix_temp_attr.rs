use crate::engine::{
    damage::modifiers,
    entity::attr::AttrId,
    manager::{
        attribute::AttributeManager,
        buff::{ActiveBuffFeature, BuffManager},
        hp::HpManager,
    },
};

use super::registry::BuffActKind;

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [output, critical_rate, critical_damage]
            if matches!(
                AttrId::from_raw(*output),
                Some(AttrId::DmgBonus | AttrId::GenesisDmgBonus)
            )
                && AttrId::from_raw(*critical_rate) == Some(AttrId::CriticalRate)
                && AttrId::from_raw(*critical_damage) == Some(AttrId::CriticalDmg)
    )
}

pub fn attribute_delta(
    feature: &ActiveBuffFeature,
    attr_id: AttrId,
    attributes: &AttributeManager,
    buffs: &BuffManager,
    hp: &HpManager,
) -> i32 {
    if !supports(feature.values.get(1..).unwrap_or_default()) {
        return 0;
    }
    let [_, output, critical_rate, critical_damage] = feature.values.as_slice() else {
        return 0;
    };
    if AttrId::from_raw(*output) != Some(attr_id) {
        return 0;
    }
    let Some(critical_rate) = AttrId::from_raw(*critical_rate) else {
        return 0;
    };
    let Some(critical_damage) = AttrId::from_raw(*critical_damage) else {
        return 0;
    };
    let owner_uid = feature.owner_uid;
    let rate = attributes.get(owner_uid, critical_rate)
        + modifiers::persistent_attribute_delta(buffs, hp, owner_uid, critical_rate);
    let damage = attributes.get(owner_uid, critical_damage)
        + modifiers::persistent_attribute_delta(buffs, hp, owner_uid, critical_damage);

    (i64::from(rate.max(0)) * i64::from((damage - 1000).max(0)) / 1000)
        .clamp(0, i64::from(i32::MAX)) as i32
}

pub fn forces_critical(
    features: &[ActiveBuffFeature],
    source_uid: i64,
    extra_action: bool,
) -> bool {
    extra_action
        && features.iter().any(|feature| {
            feature.owner_uid == source_uid
                && feature.owner_alive
                && feature.amount > 0
                && super::is_kind(feature, BuffActKind::MustCritAndFixTempAttr)
                && supports(feature.values.get(1..).unwrap_or_default())
        })
}

pub fn active_forces_critical(
    source_uid: i64,
    managers: &crate::engine::manager::BattleManagers,
    extra_action: bool,
) -> bool {
    extra_action
        && forces_critical(
            &managers.buff.active_features(&managers.hp),
            source_uid,
            true,
        )
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroExAttribute};

    use super::*;

    #[test]
    fn extra_action_uses_the_configured_critical_attributes() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31050186),
                        from_uid: Some(20),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
        managers.attribute.override_ex(
            10,
            &HeroExAttribute {
                cri: Some(400),
                cri_dmg: Some(1750),
                ..Default::default()
            },
        );
        let features = managers.buff.active_features(&managers.hp);
        let configured = features
            .iter()
            .find(|active| {
                super::super::is_kind(active, BuffActKind::MustCritAndFixTempAttr)
                    && active.values.get(1) == Some(&205)
            })
            .unwrap();

        assert_eq!(
            attribute_delta(
                configured,
                AttrId::DmgBonus,
                &managers.attribute,
                &managers.buff,
                &managers.hp,
            ),
            300
        );
        assert_eq!(
            attribute_delta(
                configured,
                AttrId::GenesisDmgBonus,
                &managers.attribute,
                &managers.buff,
                &managers.hp,
            ),
            0
        );
        assert!(!forces_critical(&features, 10, false));
        assert!(forces_critical(&features, 10, true));
        assert!(!active_forces_critical(10, &managers, false));
        assert!(active_forces_critical(10, &managers, true));
        assert_eq!(
            super::super::calculated_attack_attribute_delta_for_skill(
                configured,
                AttrId::DmgBonus,
                &managers.attribute,
                &managers.buff,
                &managers.hp,
                false,
                false,
                true,
                true,
            ),
            300
        );
        assert!(forces_critical(&features, 10, true));

        let mut malformed = configured.clone();
        malformed.values = vec![860, 205, 999, 203];
        assert_eq!(
            attribute_delta(
                &malformed,
                AttrId::DmgBonus,
                &managers.attribute,
                &managers.buff,
                &managers.hp,
            ),
            0
        );
        assert!(!forces_critical(&[malformed], 10, true));
    }
}
