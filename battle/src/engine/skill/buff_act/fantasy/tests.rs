use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

use super::*;
use crate::engine::{
    manager::{
        BattleManagers,
        gauge::{GaugeCommand, GaugeOperation},
    },
    runtime::{
        determinism::RoundDeterminism,
        drain::{self, DrainResult, ReactionLane},
    },
    skill::{
        effect::SkillEffectCatalog,
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
        target::{TargetContext, TargetPool},
    },
};

fn entity(uid: i64, team_type: i32, current_hp: i32, buffs: Vec<BuffInfo>) -> FightEntityInfo {
    FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        entity_type: Some(1),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(1_000),
            ..Default::default()
        }),
        buffs,
        ..Default::default()
    }
}

fn fight(teeth: i32) -> Fight {
    Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                entity(
                    10,
                    1,
                    500,
                    vec![
                        BuffInfo {
                            uid: Some(20),
                            buff_id: Some(30530201),
                            from_uid: Some(10),
                            layer: Some(teeth),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(21),
                            buff_id: Some(30530207),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                    ],
                ),
                entity(11, 1, 800, Vec::new()),
                emitter::activation_entity(),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                entity(-1, 2, 1_000, Vec::new()),
                entity(-2, 2, 1_000, Vec::new()),
            ],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn run_round_start(teeth: i32) -> (BattleManagers, DrainResult) {
    crate::test_support::init_config();
    let fight = fight(teeth);
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    managers
        .execute_gauge(GaugeCommand::new(
            CommandOrigin {
                domain: RuleDomain::Lifecycle,
                key: DefinitionKey::new(0, "TestSetup"),
            },
            impromptu::inspiration_key(emitter::UID),
            GaugeOperation::Enable { max: None },
        ))
        .unwrap();
    let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [30530207]);
    let result = drain::run_group_event(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        BattleEvent::RoundStart,
        ReactionLane::BuffActs,
        Some(&[10]),
    )
    .unwrap();
    (managers, result)
}

#[test]
fn grammar_uses_the_configured_cap_to_parse_bonus_buffs() {
    let raw = [
        "30530201",
        "5",
        "202",
        "30530202",
        "6",
        "30530601,30530602,30530603,30530604,30530605",
        "101",
        "1",
        "99",
        "50",
        "2",
    ]
    .map(str::to_owned);
    let parsed = parse(&raw).unwrap();

    assert!(supports(&parsed));
    assert_eq!(
        super::super::registry::destination(1086, "ToothFairyFantasy", &parsed),
        Some(super::super::registry::BuffActDestination::Runtime)
    );
    let wire = super::super::registry::find(1086, "ToothFairyFantasy")
        .unwrap()
        .wire
        .unwrap();
    assert_eq!(
        wire.markers(super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::None as i32]
    );
    assert!(
        wire.markers(super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert!(
        wire.markers(super::super::wire::WirePhase::Refresh)
            .is_empty()
    );

    let mut wrong_count = raw;
    wrong_count[5] = "4150001".to_owned();
    assert!(parse(&wrong_count).is_none());

    let mut unsupported_mode = wrong_count;
    unsupported_mode[5] = "30530601,30530602,30530603,30530604,30530605".to_owned();
    unsupported_mode[7] = "0".to_owned();
    assert!(parse(&unsupported_mode).is_none());
}

#[test]
fn round_start_caps_teeth_and_keeps_results_in_the_causing_frame() {
    let (managers, result) = run_round_start(6);
    let [root] = result.frames.as_slice() else {
        panic!("expected one causing frame")
    };
    let subscriber = root
        .items
        .iter()
        .find_map(|item| match item {
            crate::engine::runtime::record::FrameItem::Child(child)
                if matches!(
                    child.owner,
                    crate::engine::runtime::record::FrameOwner::BuffAct { key, .. }
                        if key == DefinitionKey::new(1086, "ToothFairyFantasy")
                ) =>
            {
                Some(child)
            }
            _ => None,
        })
        .expect("expected the ToothFairyFantasy subscriber frame");
    assert!(matches!(
        subscriber.items.as_slice(),
        [crate::engine::runtime::record::FrameItem::Change(change)]
            if matches!(
                change.as_ref(),
                crate::engine::runtime::change::BattleChange::Buff(changes)
                    if changes.change.refreshed.iter().any(|refresh|
                        refresh.after.buff_id == Some(30530201)
                            && refresh.after.layer == Some(1)
                    )
            )
    ));
    let direct_changes = root
        .items
        .iter()
        .filter_map(|item| match item {
            crate::engine::runtime::record::FrameItem::Change(change) => Some(change.as_ref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        direct_changes
            .iter()
            .filter(|change| matches!(
                change,
                crate::engine::runtime::change::BattleChange::Buff(_)
            ))
            .count(),
        7
    );
    assert_eq!(
        direct_changes
            .iter()
            .filter(|change| matches!(change, crate::engine::runtime::change::BattleChange::Hp(_)))
            .count(),
        2
    );
    assert_eq!(
        direct_changes
            .iter()
            .filter(|change| matches!(
                change,
                crate::engine::runtime::change::BattleChange::Gauge(_)
            ))
            .count(),
        1
    );

    assert_eq!(managers.buff.buff_id_amount(10, 30530201), 1);
    assert!(managers.buff.has_buff_id(10, 30530207));
    for target_uid in [-1, -2] {
        assert_eq!(managers.buff.buff_id_amount(target_uid, 30530202), 5);
    }
    for buff_id in 30530601..=30530605 {
        assert!(managers.buff.has_buff_id(emitter::UID, buff_id));
    }
    assert_eq!(managers.hp.current(10), 625);
    assert_eq!(managers.hp.current(11), 850);
    assert_eq!(
        managers
            .gauge
            .get(impromptu::inspiration_key(emitter::UID))
            .unwrap()
            .current,
        10
    );
}

#[test]
fn depleted_teeth_report_zero_before_removal() {
    let (managers, result) = run_round_start(5);
    let [root] = result.frames.as_slice() else {
        panic!("expected one causing frame")
    };
    let removed = root.items.iter().find_map(|item| match item {
        crate::engine::runtime::record::FrameItem::Child(child)
            if matches!(
                child.owner,
                crate::engine::runtime::record::FrameOwner::BuffAct { key, .. }
                    if key == DefinitionKey::new(1086, "ToothFairyFantasy")
            ) =>
        {
            child.items.iter().find_map(|item| match item {
                crate::engine::runtime::record::FrameItem::Change(change) => {
                    let crate::engine::runtime::change::BattleChange::Buff(changes) =
                        change.as_ref()
                    else {
                        return None;
                    };
                    changes
                        .change
                        .removed
                        .iter()
                        .find(|removed| removed.buff.buff_id == Some(30530201))
                }
                _ => None,
            })
        }
        _ => None,
    });

    assert_eq!(removed.unwrap().buff.layer, Some(0));
    assert!(!managers.buff.has_buff_id(10, 30530201));
}
