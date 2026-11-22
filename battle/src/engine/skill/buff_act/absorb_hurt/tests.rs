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
