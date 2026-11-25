use super::*;
use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffGrant},
        gauge::{GaugeCommand, GaugeOperation},
        hp::{HpCommand, HpKill},
    },
    round::outcome::terminal_outcome_for_battle_id,
    runtime::schedule::start::{deal_round_start, settle_round_start},
};
use sonettobuf::effect_type_enum::EffectType;

const THRESHOLD_ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::BuffAct,
    key: DefinitionKey::new(1021, "BloodPoolCountAddExPoint"),
};

fn fight() -> Fight {
    let entity = |uid, team_type| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        ex_point: Some(4),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                ex_skill: Some(900),
                skill_group1: vec![100],
                ..entity(10, 1)
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(20, 2)],
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn seed_threshold(managers: &mut BattleManagers, uid: i64, team: i32) -> i64 {
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: THRESHOLD_ORIGIN,
            source_uid: uid,
            target_uid: uid,
            buff_id: 308802111,
            amount: Some(1),
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    let key = crate::engine::mechanic::bloodtithe::rule::key(team);
    for operation in [
        GaugeOperation::Enable { max: Some(56) },
        GaugeOperation::ChangeValue { delta: 8 },
    ] {
        managers
            .execute_gauge(GaugeCommand::new(THRESHOLD_ORIGIN, key, operation))
            .unwrap();
    }

    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| feature.owner_uid == uid && feature.buff_id == 308802111)
        .unwrap()
        .buff_uid
}

#[test]
fn bloodtithe_threshold_pays_after_normal_refill() {
    init_config();
    let fight = fight();
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    seed_threshold(&mut managers, 10, 1);
    let card = CardInfo {
        uid: Some(10),
        skill_id: Some(100),
        ..Default::default()
    };
    managers
        .execute_card(CardCommand::Setup(CardSetup {
            hand: Vec::new(),
            draw_pile: vec![card.clone()],
            deck_num: 1,
        }))
        .unwrap();

    let mut determinism = RoundDeterminism::default();
    let context = TargetContext {
        current_round: 3,
        ..Default::default()
    };
    let started = settle_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        &[],
    )
    .unwrap();
    assert_eq!(managers.ex_point.get(10), 4);

    let (_, next_round, _, dealt_cards) = deal_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        started,
        1,
    )
    .unwrap();
    assert_eq!(managers.ex_point.get(10), 5);
    assert_eq!(
        dealt_cards
            .iter()
            .filter_map(|card| card.skill_id)
            .collect::<Vec<_>>(),
        [100]
    );
    assert_eq!(managers.card.hand().len(), 1);
    assert_eq!(managers.card.hand()[0].skill_id, Some(100));

    let steps =
        crate::engine::packet::timeline::project_for_version(&next_round.frames, 7).unwrap();
    let [deal, payout, push, ..] = steps.as_slice() else {
        panic!("expected a deal, threshold payout and hand push");
    };
    assert_eq!(
        deal.act_effect[0].effect_type,
        Some(EffectType::Dealcard1 as i32)
    );
    let child = payout.act_effect[0].fight_step.as_ref().unwrap();
    assert_eq!(child.act_effect[0].buff_act_id, Some(1021));
    assert_eq!(
        child.act_effect[1].effect_type,
        Some(EffectType::Expointchange as i32)
    );
    assert_eq!(child.act_effect[1].effect_num, Some(1));
    assert_eq!(
        push.act_effect[0].effect_type,
        Some(EffectType::Cardspush as i32)
    );
}

#[test]
fn bloodtithe_threshold_keeps_only_winning_settlement_after_defeat() {
    init_config();
    let fight = fight();
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let losing_buff_uid = seed_threshold(&mut managers, 10, 1);
    seed_threshold(&mut managers, 20, 2);
    let mut determinism = RoundDeterminism::default();
    let context = TargetContext {
        current_round: 3,
        ..Default::default()
    };
    let started = settle_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        &[],
    )
    .unwrap();
    managers
        .execute_hp(HpCommand::Kill(HpKill {
            origin: CommandOrigin {
                domain: RuleDomain::Lifecycle,
                key: DefinitionKey::new(0, "TestDeath"),
            },
            source_uid: 20,
            target_uid: 10,
            config_effect: 1,
        }))
        .unwrap();
    let defeat = terminal_outcome_for_battle_id(context.battle_id, &pool, &managers).unwrap();
    assert_eq!(defeat.winning_team(), Some(2));
    assert!(managers.commit_terminal(defeat));

    let (settlement, next_round, _, _) = deal_round_start(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        context,
        started,
        0,
    )
    .unwrap();

    assert_eq!(
        managers.gauge.preview_positive_threshold(
            crate::engine::mechanic::bloodtithe::rule::key(1),
            losing_buff_uid,
            1021,
            8,
            1,
        ),
        1
    );
    assert_eq!(managers.ex_point.get(10), 4);
    assert_eq!(managers.ex_point.get(20), 5);
    assert!(settlement.events.iter().any(|event| matches!(
        event,
        BattleEvent::ExPointChanged(change) if change.target_uid == 20
    )));
    assert!(
        !next_round
            .events
            .iter()
            .any(|event| matches!(event, BattleEvent::ExPointChanged(_)))
    );
}
