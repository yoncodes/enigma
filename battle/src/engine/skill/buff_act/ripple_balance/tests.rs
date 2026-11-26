use sonettobuf::{BuffActInfo, BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use crate::engine::{
    manager::{
        BattleManagers,
        hp::{
            DamageEffectKind, HpCommand, HpCommandError, HpDamage, HurtDamageFromType, HurtInfoData,
        },
    },
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

#[test]
fn ripple_negates_smaller_hits_and_breaks_on_an_equal_hit() {
    crate::test_support::init_config();
    let unit = |uid, team| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team),
        current_hp: Some(10_000),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                buffs: vec![BuffInfo {
                    uid: Some(100),
                    buff_id: Some(31572001),
                    from_uid: Some(10),
                    act_info: vec![BuffActInfo {
                        act_id: Some(1153),
                        param: vec![800],
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..unit(10, 1)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![unit(-1, 2)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let hit = |target_uid, amount| {
        HpCommand::Damage(HpDamage {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(0, "TestIncomingAttack"),
            },
            source_uid: -1,
            target_uid,
            amount,
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
                hurt_effect_type: sonettobuf::effect_type_enum::EffectType::Damage as i32,
                display_amount: None,
            },
        })
    };

    let small = managers.execute_rule_hp(hit(10, 799)).unwrap();
    assert_eq!(small.changes.applied_damage(), 0);
    assert_eq!(managers.hp.current(10), 10_000);
    assert!(small.before_hp_markers.is_empty());
    assert_eq!(
        managers.buff.snapshot(10, 100).unwrap().act_info[0].param,
        [800]
    );

    managers.hp.add_shield(10, 30, 30);
    assert!(matches!(
        managers.execute_rule_hp(hit(10, 800)),
        Err(HpCommandError::UnsupportedRippleInteraction(10))
    ));
    assert_eq!(managers.hp.current(10), 10_000);
    assert_eq!(managers.hp.shield(10), 30);
    managers.hp.absorb_shield(10, 30);

    assert!(
        managers
            .execute_rule_hp_batch(vec![hit(10, 800), hit(10, -1)])
            .is_err()
    );
    assert_eq!(managers.hp.current(10), 10_000);
    assert_eq!(
        managers.buff.snapshot(10, 100).unwrap().act_info[0].param,
        [800]
    );

    let batch = managers
        .execute_rule_hp_batch(vec![hit(10, 800), hit(10, 800)])
        .unwrap();
    let [equal, next] = batch.as_slice() else {
        panic!("expected both staged hits");
    };
    assert_eq!(equal.before_hp_markers[0].params, [0]);
    assert_eq!(equal.changes.applied_damage(), 560);
    assert_eq!(next.changes.applied_damage(), 800);
    assert_eq!(managers.hp.current(10), 8_640);
    let removed = equal.changes.post_damage_expiry.as_ref().unwrap();
    assert_eq!(removed.change.removed[0].buff.act_info[0].param, [0]);
    assert!(managers.buff.snapshot(10, 100).is_none());

    let mut shielded = fight;
    shielded
        .attacker
        .as_mut()
        .unwrap()
        .entitys
        .push(FightEntityInfo {
            buffs: vec![BuffInfo {
                uid: Some(101),
                buff_id: Some(31430144),
                from_uid: Some(11),
                act_info: vec![BuffActInfo {
                    act_id: Some(1125),
                    param: vec![100],
                    str_param: Some(String::new()),
                }],
                ..Default::default()
            }],
            ..unit(11, 1)
        });
    let mut managers = BattleManagers::seeded(&shielded);
    let batch = managers
        .execute_rule_hp_batch(vec![hit(11, 800), hit(10, 800)])
        .unwrap();
    let [shield_hit, ripple_hit] = batch.as_slice() else {
        panic!("expected both staged hits");
    };
    assert_eq!(
        shield_hit
            .changes
            .team_shared_shield_absorbed
            .unwrap()
            .after,
        0
    );
    assert!(ripple_hit.changes.team_shared_shield_absorbed.is_none());
    assert_eq!(ripple_hit.changes.applied_damage(), 560);
    assert!(managers.buff.snapshot(10, 100).is_none());
}
