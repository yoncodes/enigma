use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    manager::{
        BattleManagers,
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{buff_act::registry::BuffActKind, target::TargetPool},
};

pub fn route(managers: &BattleManagers, pool: &TargetPool, command: HpCommand) -> Vec<HpCommand> {
    let HpCommand::Damage(mut damage) = command else {
        return vec![command];
    };
    let absorber = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid != damage.target_uid
                && feature.owner_alive
                && pool
                    .main_allies(damage.target_uid)
                    .iter()
                    .any(|ally| ally.uid == feature.owner_uid)
                && super::is_kind(feature, BuffActKind::AbsorbHurt)
        })
        .find(|feature| {
            super::injury_bank::state(managers, feature.owner_uid)
                .is_some_and(|state| state.current < state.cap)
        });
    let Some(feature) = absorber else {
        return vec![HpCommand::Damage(damage)];
    };
    let [_, rate, ..] = feature.values.as_slice() else {
        return vec![HpCommand::Damage(damage)];
    };
    if !(1..=1000).contains(rate) {
        return vec![HpCommand::Damage(damage)];
    }
    let absorbed = damage.amount * *rate / 1000;
    if absorbed <= 0 {
        return vec![HpCommand::Damage(damage)];
    }
    damage.amount -= absorbed;
    vec![
        HpCommand::Lose(HpLoss {
            origin: damage.origin,
            source_uid: damage.source_uid,
            target_uid: feature.owner_uid,
            amount: absorbed,
            config_effect: damage.config_effect,
            hurt: Some(HurtInfoData {
                from_uid: damage.source_uid,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 0,
                damage_from: HurtDamageFromType::AbsorbHurt,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: EffectType::Damagefromabsorb as i32,
                display_amount: None,
            }),
        }),
        HpCommand::Damage(damage),
    ]
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        manager::hp::{DamageEffectKind, HpDamage},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    #[test]
    fn redirects_the_configured_share_before_the_ally_damage() {
        crate::test_support::init_config();
        let entity = |uid, buffs| FightEntityInfo {
            uid: Some(uid),
            team_type: Some(1),
            current_hp: Some(1_000),
            attr: Some(HeroAttribute {
                hp: Some(1_000),
                ..Default::default()
            }),
            buffs,
            ..Default::default()
        };
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    entity(10, Vec::new()),
                    entity(
                        11,
                        vec![
                            BuffInfo {
                                uid: Some(20),
                                buff_id: Some(30800131),
                                from_uid: Some(11),
                                ..Default::default()
                            },
                            BuffInfo {
                                uid: Some(21),
                                buff_id: Some(30800141),
                                from_uid: Some(11),
                                act_common_params: Some("770#0#300".to_owned()),
                                ..Default::default()
                            },
                        ],
                    ),
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let origin = CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "SkillDamage"),
        };
        let command = HpCommand::Damage(HpDamage {
            origin,
            source_uid: -1,
            target_uid: 10,
            amount: 400,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: -1,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 1,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: EffectType::Damage as i32,
                display_amount: None,
            },
        });

        let routed = route(&managers, &pool, command);

        assert!(matches!(
            routed.as_slice(),
            [
                HpCommand::Lose(HpLoss {
                    target_uid: 11,
                    amount: 200,
                    hurt: Some(HurtInfoData {
                        damage_from: HurtDamageFromType::AbsorbHurt,
                        ..
                    }),
                    ..
                }),
                HpCommand::Damage(HpDamage {
                    target_uid: 10,
                    amount: 200,
                    ..
                })
            ]
        ));
    }
}
