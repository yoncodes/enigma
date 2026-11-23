use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;

const OWNER: i64 = 10;
const BUFF_UID: i64 = 30;

fn managers(current_hp: i32, stored: Option<i32>) -> BattleManagers {
    crate::test_support::init_config();
    BattleManagers::seeded(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(OWNER),
                team_type: Some(1),
                current_hp: Some(current_hp),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(BUFF_UID),
                    buff_id: Some(108_300_002),
                    from_uid: Some(0),
                    layer: Some(1),
                    act_info: stored
                        .map(|value| BuffActInfo {
                            act_id: Some(1059),
                            param: vec![value],
                            str_param: Some(String::new()),
                        })
                        .into_iter()
                        .collect(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    })
}

fn feature(managers: &BattleManagers) -> ActiveBuffFeature {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| {
            super::super::is_kind(feature, super::super::registry::BuffActKind::FictionHp)
        })
        .expect("the Dissonance buff carries the fiction HP act")
}

fn round_start_ops(managers: &BattleManagers) -> Vec<BuffActRuleOp> {
    let subscriber =
        super::super::subscriber_from_feature(feature(managers), EventKind::RoundStart)
            .expect("fiction HP subscribes to round start");
    rule_ops(
        managers,
        &subscriber,
        &BattleEvent::Kind(EventKind::RoundStart),
    )
    .expect("fiction HP handles round start")
}

#[test]
fn hp_loss_stores_and_publishes_the_remaining_hp_after_the_hit() {
    let managers = managers(1_000, None);

    let updates = hp_loss_state_update(&managers, &feature(&managers), 300).unwrap();

    let [update] = updates.as_slice() else {
        panic!("expected one state update, got {updates:?}")
    };
    assert!(update.after_hp);
    assert_eq!(update.marker.act_id, 1059);
    assert_eq!(update.marker.buff_uid, BUFF_UID);
    assert_eq!(update.marker.params, [700]);
    let BuffCommand::SetInternalState(BuffSetState {
        act_info: Some(act_info),
        ..
    }) = &update.command
    else {
        panic!("expected the act state to be stored")
    };
    assert_eq!(act_info[0].param, [700]);
}

#[test]
fn round_start_trims_hp_restored_above_the_stored_value() {
    let ops = round_start_ops(&managers(900, Some(700)));

    let [
        BuffActRuleOp {
            op: RuleOp::Command(BattleCommand::Hp(HpCommand::SetCurrent(set))),
            ..
        },
    ] = ops.as_slice()
    else {
        panic!("expected one current HP reset, got {ops:?}")
    };
    assert_eq!((set.target_uid, set.value), (OWNER, 700));
}

#[test]
fn round_start_leaves_unhit_or_unhealed_owners_alone() {
    assert!(round_start_ops(&managers(900, None)).is_empty());
    assert!(round_start_ops(&managers(700, Some(700))).is_empty());
    assert!(round_start_ops(&managers(500, Some(700))).is_empty());
    assert!(round_start_ops(&managers(500, Some(0))).is_empty());
}

#[test]
fn a_hit_publishes_after_the_hp_change_and_the_trim_is_not_a_hit() {
    use crate::engine::manager::hp::{
        DamageEffectKind, HpDamage, HurtDamageFromType, HurtInfoData,
    };

    let mut managers = managers(900, Some(900));
    let origin = super::super::feature_command_origin(&feature(&managers)).unwrap();

    let hit = managers
        .execute_rule_hp(HpCommand::Damage(HpDamage {
            origin,
            source_uid: 99,
            target_uid: OWNER,
            amount: 200,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: 99,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 1,
                skill_id: 1,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            },
        }))
        .unwrap();
    assert!(hit.before_hp_markers.is_empty());
    assert_eq!(hit.after_hp_markers.len(), 1);
    assert_eq!(hit.after_hp_markers[0].params, [700]);

    let trim = managers
        .execute_rule_hp(HpCommand::SetCurrent(CurrentHpSet {
            origin,
            source_uid: 0,
            target_uid: OWNER,
            value: 600,
            config_effect: 0,
            effect_type: EffectType::Currenthpchange as i32,
        }))
        .unwrap();
    assert_eq!(managers.hp.current(OWNER), 600);
    assert!(trim.before_hp_markers.is_empty());
    assert!(trim.after_hp_markers.is_empty());
    assert_eq!(trim.changes.applied_damage(), 0);
    assert_eq!(managers.hp.take_damage_taken(), [(OWNER, 200)]);
    assert!(
        trim.changes
            .events()
            .iter()
            .all(|event| !matches!(event, BattleEvent::HpLost { .. }))
    );
}
