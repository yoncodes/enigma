use crate::engine::{
    entity::attr::AttrId,
    manager::{buff::ActiveBuffFeature, hp::HpManager},
};

pub fn attribute_delta(feature: &ActiveBuffFeature, attr_id: AttrId, hp: &HpManager) -> i32 {
    let parts = feature.raw.split('#').collect::<Vec<_>>();
    let Some((step, raw_attrs, raw_values, max_steps, absolute_step)) = (match parts.as_slice() {
        [_, step, attrs, values, max_steps] => Some((*step, *attrs, *values, *max_steps, false)),
        [_, step, attr, value, max_steps, "1", mode] if matches!(*mode, "0" | "1") => {
            Some((*step, *attr, *value, *max_steps, true))
        }
        _ => None,
    }) else {
        return 0;
    };
    let Ok(step) = step.parse::<i32>() else {
        return 0;
    };
    let Ok(max_steps) = max_steps.parse::<i32>() else {
        return 0;
    };
    if step <= 0 || max_steps <= 0 {
        return 0;
    }
    let Some(value) =
        raw_attrs
            .split(',')
            .zip(raw_values.split(','))
            .find_map(|(raw_attr, raw_value)| {
                (raw_attr.parse().ok().and_then(AttrId::from_raw) == Some(attr_id))
                    .then(|| raw_value.parse::<i32>().ok())
                    .flatten()
            })
    else {
        return 0;
    };
    let state = hp.get(feature.owner_uid);
    if state.max <= 0 {
        return 0;
    }
    let missing_hp = (state.max - state.current).max(0);
    let missing = if absolute_step {
        missing_hp
    } else {
        ((i64::from(missing_hp) * 1000) / i64::from(state.max)) as i32
    };
    value * (missing / step).min(max_steps)
}

pub fn supports(args: &[i32]) -> bool {
    if let [step, attr, _, max_steps, 1, mode] = args {
        return *step > 0
            && AttrId::from_raw(*attr).is_some()
            && *max_steps > 0
            && matches!(*mode, 0 | 1);
    }
    matches!(args, [step, first_attr, second_attr, .., max_steps]
        if *step > 0
            && AttrId::from_raw(*first_attr).is_some()
            && AttrId::from_raw(*second_attr).is_some()
            && *max_steps > 0)
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;

    #[test]
    fn configured_attributes_scale_by_missing_hp_and_cap() {
        let mut hp = HpManager::default();
        hp.seed(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let feature = ActiveBuffFeature {
            owner_uid: 1,
            source_uid: 1,
            buff_uid: 1,
            buff_id: 1,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "AttrByLostHp".into(),
            effect_time: 203,
            effect_condition: 0,
            raw: "853#100#205,206#25,25#8".into(),
            values: vec![853, 100, 205, 206, 25, 25, 8],
        };

        assert_eq!(attribute_delta(&feature, AttrId::DmgBonus, &hp), 200);
        assert_eq!(
            attribute_delta(&feature, AttrId::DmgTakenReduction, &hp),
            200
        );
    }

    #[test]
    fn absolute_missing_hp_steps_scale_playmode_attributes() {
        let mut hp = HpManager::default();
        hp.seed(&Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(195_500_000),
                    attr: Some(HeroAttribute {
                        hp: Some(200_000_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let feature = |raw: &str, act_type: &str| ActiveBuffFeature {
            owner_uid: -1,
            source_uid: -1,
            buff_uid: 1,
            buff_id: 1,
            amount: 1,
            team_type: 2,
            owner_alive: true,
            act_type: act_type.into(),
            effect_time: 203,
            effect_condition: 0,
            raw: raw.into(),
            values: raw
                .split('#')
                .filter_map(|value| value.parse().ok())
                .collect(),
        };

        assert_eq!(
            attribute_delta(
                &feature("853#4000000#215#400#1#1#0", "AttrByLostHp"),
                AttrId::PlaymodeDmgIncrease,
                &hp,
            ),
            400
        );
        assert_eq!(
            attribute_delta(
                &feature("1056#2000000#216#200#1#1#1", "AttrByLostHp"),
                AttrId::PlaymodeDmgImmunity,
                &hp,
            ),
            200
        );
    }
}
