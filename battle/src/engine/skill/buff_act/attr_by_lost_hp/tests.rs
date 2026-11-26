use crate::engine::runtime::drain::BattleAccess;
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

#[test]
fn absolute_outgoing_evaluation_tracks_committed_defender_cards() {
    use sonettobuf::BuffInfo;

    use crate::engine::{
        manager::BattleManagers,
        runtime::{
            determinism::{AiSkillChoice, RoundDeterminism},
            drain,
            schedule::run_ai_actions,
        },
        skill::{
            action::{SkillInvocation, SkillRequest},
            effect::SkillEffectCatalog,
            target::{TargetContext, TargetPool},
        },
    };

    crate::test_support::init_config();
    let holder = FightEntityInfo {
        uid: Some(-1),
        team_type: Some(2),
        current_hp: Some(97_000_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000_000),
            ..Default::default()
        }),
        buffs: vec![BuffInfo {
            uid: Some(20),
            buff_id: Some(109320104),
            from_uid: Some(-1),
            duration: Some(-1),
            ..Default::default()
        }],
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                holder.clone(),
                FightEntityInfo {
                    uid: Some(-2),
                    team_type: Some(2),
                    current_hp: Some(100),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let self_consumption = 116330112;
    let strike = 103374111;
    let catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [self_consumption, strike], []);
    let play_card = |managers: &mut BattleManagers, source_uid| {
        run_ai_actions(
            &fight,
            BattleAccess::new(
                managers,
                &pool,
                &catalog,
                &mut RoundDeterminism::default(),
                TargetContext::default(),
            ),
            [AiSkillChoice {
                source_uid,
                skill_id: self_consumption,
                target_uid: source_uid,
            }],
            &[],
        )
        .unwrap();
    };
    let outgoing = AttrId::PlaymodeDmgIncrease;
    let incoming = AttrId::PlaymodeDmgImmunity;

    play_card(&mut managers, -1);
    managers.hp.register(&FightEntityInfo {
        current_hp: Some(95_000_000),
        ..holder
    });
    assert_eq!(managers.settled_playmode_increase(-1), 0);
    assert_eq!(managers.origin_attribute(-1, incoming), 200);
    assert_eq!(managers.persistent_attribute_delta(-1, outgoing), 400);

    play_card(&mut managers, -1);
    assert_eq!(managers.settled_playmode_increase(-1), 400);

    let attacker = &fight.attacker.as_ref().unwrap().entitys[0];
    managers.hp.register(&FightEntityInfo {
        current_hp: Some(0),
        ..attacker.clone()
    });
    let mut rejected: SkillInvocation = SkillRequest {
        source_uid: -2,
        skill_id: strike,
    }
    .into();
    rejected.card_index = 1;
    let result = drain::run_round_action(
        BattleAccess::new(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
        ),
        [],
        None,
        rejected,
    )
    .unwrap();
    assert!(result.frames.is_empty());
    assert_eq!(managers.settled_playmode_increase(-1), 400);

    managers.hp.register(attacker);
    play_card(&mut managers, -2);
    assert_eq!(managers.settled_playmode_increase(-1), 0);
    assert_eq!(managers.origin_attribute(-1, incoming), 200);
    assert_eq!(managers.persistent_attribute_delta(-1, outgoing), 400);
}
