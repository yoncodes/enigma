use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    manager::{
        card::{CARD_PLAY_ORIGIN, CardCommand, CardPlay, CardSetup},
        conduit::{ConduitCommand, ConduitPowerChange, ConduitPowerChangeKind},
    },
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
};

const LIGHT: i32 = 31450181;

fn setup() -> (BattleManagers, TargetPool, SkillEffectCatalog) {
    let entity = |uid, model, team, position| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model),
        team_type: Some(team),
        position: Some(position),
        current_hp: Some(1_000),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, 3149, 1, 1),
                entity(11, 3149, 1, 2),
                entity(12, 3145, 1, 3),
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 50001, 2, 1)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let catalog = managers.catalog();
    let pool = TargetPool::from_fight_with_catalog(catalog, &fight).runtime_view(&managers);
    let mut effects = SkillEffectCatalog::from_fight(crate::test_support::game_data(), &fight);
    catalog.extend_skill_roots(
        &mut effects,
        [LIGHT, 31490121, 31490141],
        std::iter::empty(),
    );
    (managers, pool, effects)
}

#[test]
fn estimates_mixed_payments_from_current_and_queued_energy_in_device_order() {
    let (mut managers, pool, catalog) = setup();
    managers
        .conduit
        .execute(ConduitCommand::ChangePower(ConduitPowerChange {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60291, "AddDevicePower"),
            },
            source_uid: 10,
            team: 1,
            power_id: 1,
            delta: 1,
            kind: ConduitPowerChangeKind::Standard,
        }))
        .unwrap();
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: vec![
                CardInfo {
                    uid: Some(12),
                    skill_id: Some(LIGHT),
                    ..Default::default()
                };
                2
            ],
            draw_pile: Vec::new(),
            deck_num: 0,
        }))
        .unwrap();
    let prefix = [BeginRoundOper {
        oper_type: Some(CardOpType::PlayCard.id()),
        param1: Some(1),
        to_id: Some(-1),
        ..Default::default()
    }];
    for _ in 0..2 {
        managers
            .execute_card(CardCommand::Play(CardPlay {
                origin: CARD_PLAY_ORIGIN,
                hand_index: 0,
                target_uid: Some(-1),
                chosen_skill_id: None,
                choice: None,
                recorded_skill: None,
            }))
            .unwrap();
    }
    let mut devices = [
        FightDeviceOper {
            uid: Some(10),
            index: Some(2),
        },
        FightDeviceOper {
            uid: Some(11),
            index: Some(1),
        },
    ];
    let quote = |devices: &[FightDeviceOper]| {
        estimate(
            &managers,
            &pool,
            &catalog,
            &RoundDeterminism::default(),
            TargetContext::default(),
            devices,
            &prefix,
            managers.card.played(),
        )
        .unwrap()
    };
    let forward = quote(&devices);
    assert_eq!(forward.current, [((1, 1), 1)].into());
    assert_eq!(forward.queued, [((1, 0), 2)].into());
    assert_eq!(
        forward.first_pass,
        [
            (10, 31490141, Ok([(2, 0), (0, 1)])),
            (
                11,
                31490121,
                Err(ConduitError::InsufficientPower {
                    power_id: 1,
                    available: 2,
                    required: 3
                })
            ),
        ]
    );

    devices.reverse();
    assert_eq!(
        quote(&devices).first_pass,
        [
            (11, 31490121, Ok([(1, 1), (0, 2)])),
            (
                10,
                31490141,
                Err(ConduitError::InsufficientPower {
                    power_id: 2,
                    available: 0,
                    required: 1
                })
            ),
        ]
    );
    assert_eq!(managers.conduit.power(1, 1), 1);
    assert_eq!(managers.conduit.power(1, 0), 0);
}

#[test]
fn estimates_require_a_verified_prefix() {
    let (managers, pool, catalog) = setup();
    let mut play = PlayedCard {
        card: CardInfo {
            uid: Some(12),
            skill_id: Some(31450111),
            ..Default::default()
        },
        caster_uid: 12,
        card_index: 1,
        hand_index: Some(0),
        skill_id: LIGHT,
        uses_action_point: true,
        rank_change_pending: false,
        rewritten: false,
        target_uid: Some(-1),
        recorded_skill: None,
    };
    let result = |prefix: &[BeginRoundOper], card: PlayedCard| {
        estimate(
            &managers,
            &pool,
            &catalog,
            &RoundDeterminism::default(),
            TargetContext::default(),
            &[],
            prefix,
            &[card],
        )
        .err()
    };
    assert_eq!(result(&[], play.clone()), Some("unverified card choice"));
    play.card.skill_id = Some(LIGHT);
    assert_eq!(
        result(
            &[BeginRoundOper {
                oper_type: Some(CardOpType::AssistBoss.id()),
                param1: Some(104335001),
                ..Default::default()
            }],
            play.clone()
        ),
        Some("unverified assist in prefix")
    );
    assert_eq!(
        result(
            &[BeginRoundOper {
                oper_type: Some(CardOpType::PlayCard.id()),
                param1: Some(1),
                param2: Some(2),
                ..Default::default()
            }],
            play
        ),
        Some("unresolved discard in prefix")
    );
}

#[test]
fn fixed_gain_does_not_accept_extra_raw_arguments() {
    let (_, _, catalog) = setup();
    let mut slot = catalog
        .get(LIGHT)
        .unwrap()
        .slots
        .iter()
        .find(|slot| fixed_conduit_gain(slot).is_some())
        .unwrap()
        .clone();
    assert_eq!(fixed_conduit_gain(&slot), Some((0, 1)));
    slot.behavior.raw_args.push("invalid".to_owned());
    assert_eq!(fixed_conduit_gain(&slot), None);
}
