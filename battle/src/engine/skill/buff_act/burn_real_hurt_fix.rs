use crate::engine::manager::BattleManagers;

use super::{is_kind, registry::BuffActKind};

pub fn adjusted_amount(managers: &BattleManagers, owner_uid: i64, amount: i32) -> i32 {
    let mut reduction = 0i32;
    let mut floor = 0i32;
    for feature in managers.buff.active_features(&managers.hp) {
        if feature.owner_uid != owner_uid || !is_kind(&feature, BuffActKind::BurnRealHurtFix) {
            continue;
        }
        let [_, feature_reduction, feature_floor] = feature.values.as_slice() else {
            continue;
        };
        reduction = reduction.saturating_add(*feature_reduction);
        floor = floor.max(*feature_floor);
    }

    let scaled = (i64::from(amount) * i64::from(1000i32.saturating_sub(reduction).max(0)) / 1000)
        .clamp(0, i64::from(i32::MAX)) as i32;
    let hp = managers.hp.get(owner_uid);
    let minimum = i64::from(hp.max) * i64::from(floor) / 1000;
    scaled.min((i64::from(hp.current) - minimum).clamp(0, i64::from(i32::MAX)) as i32)
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;

    fn managers(current_hp: i32) -> BattleManagers {
        crate::test_support::init_config();
        BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(current_hp),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(30940151),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    #[test]
    fn joe_halves_burn_damage_without_crossing_the_configured_hp_floor() {
        assert_eq!(adjusted_amount(&managers(3_000), 10, 1_000), 500);
        assert_eq!(adjusted_amount(&managers(1_800), 10, 1_000), 300);
    }
}
