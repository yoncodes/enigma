use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::payload::BattleEvent,
    manager::{BattleManagers, eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand}, gauge::{GaugeCommand, GaugeOperation}},
    mechanic::impromptu,
    skill::{
        action::SkillExecutionMode,
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

// The Eureka-spend transaction also grants energy for the following round.
// This feature additionally grants inspiration to the current Impromptu action.
pub fn supports(args: &[i32]) -> bool {
    matches!(args, [cost, gain] if *cost > 0 && cost == gain)
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
    catalog: &SkillEffectCatalog,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::SkillAction(action) = event else { return None; };
    let [cost, _] = subscriber.args.as_slice() else { return None; };
    let source_uid = subscriber.source_uid;
    if !supports(&subscriber.args)
        || action.source_uid != subscriber.owner_uid
        || !subscriber.owner_alive
        || action.mode != SkillExecutionMode::Active
        || action.card_index <= 0
        || !managers.card.played().iter().any(|played| {
            played.card_index == action.card_index && played.caster_uid == action.source_uid
        })
        || catalog.is_big_skill(action.skill_id)
        || managers.hp.current(source_uid) <= 0
        || managers.entity.team_type(source_uid) != Some(subscriber.team_type)
        || managers.eureka.get(source_uid, EUREKA_RESOURCE_ID).current <= 0
    {
        return Some(Vec::new());
    }
    let actual_cost = managers.eureka.get(source_uid, EUREKA_RESOURCE_ID).current.min(*cost);
    let origin = super::command_origin(subscriber)?;
    let mut ops = vec![RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(
        EurekaChange {
            origin,
            source_uid: subscriber.owner_uid,
            target_uid: source_uid,
            power_id: EUREKA_RESOURCE_ID,
            delta: -*cost,
            effect_type: EffectType::Powerchange as i32,
        },
    )))];
    let key = impromptu::inspiration_key(crate::engine::manager::emitter::UID);
    if managers.gauge.get(key).is_some() {
        ops.push(RuleOp::Command(BattleCommand::Gauge(GaugeCommand::new(
            origin, key, GaugeOperation::ChangeValue { delta: actual_cost },
        ))));
    }
    Some(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        manager::card::{CardCommand, CardPlay, CardSetup},
        skill::{action::{SkillActionEvent, SkillPhase}, rule::{CommandOrigin, DefinitionKey, RuleDomain}},
    };
    use sonettobuf::{CardInfo, Fight, FightEntityInfo, FightTeam, PowerInfo};

    fn fixture(power: i32, source_hp: i32) -> BattleManagers {
        crate::test_support::init_config();
        let mut managers = BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam { entitys: vec![
                FightEntityInfo { uid: Some(10), team_type: Some(1), current_hp: Some(source_hp),
                    power_infos: vec![PowerInfo { power_id: Some(1), num: Some(power), max: Some(10) }],
                    ..Default::default() },
                FightEntityInfo { uid: Some(11), team_type: Some(1), current_hp: Some(100), ..Default::default() },
            ], ..Default::default() }), ..Default::default()
        });
        let origin = CommandOrigin { domain: RuleDomain::Lifecycle, key: DefinitionKey::new(0, "Test") };
        managers.execute_card(CardCommand::Setup(CardSetup {
            hand: vec![CardInfo { uid: Some(11), skill_id: Some(1), ..Default::default() }],
            draw_pile: Vec::new(), deck_num: 1,
        })).unwrap();
        managers.execute_card(CardCommand::Play(CardPlay {
            origin, hand_index: 0, target_uid: Some(-1), chosen_skill_id: None,
            choice: None, recorded_skill: None,
        })).unwrap();
        managers
    }

    fn subscriber() -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: 11, source_uid: 10, buff_uid: 20, buff_id: 31410407, team_type: 1,
            owner_alive: true, amount: 1,
            key: SubscriptionKey::new(EventKind::SkillCast, DefinitionKey::new(1082, "UseSkillConsumeFromAddEmitterEnergy")),
            act_type: "UseSkillConsumeFromAddEmitterEnergy".into(), effect_time: 208,
            effect_condition: 0, args: vec![2, 2], raw: "1082#2#2".into(),
        }
    }

    fn event(mode: SkillExecutionMode, actor: i64) -> BattleEvent {
        BattleEvent::SkillAction(SkillActionEvent {
            card_index: 1,
            source_uid: actor, skill_id: 1, target_uid: -1, target_uids: vec![-1],
            attacked_target_uids: vec![-1], phase: SkillPhase::HitPassives, skill_slot: 1,
            is_attack: true, rank: 1, skill_type: 0, effect_tag: 0, assassinate: false,
            ignore_riposte: false, damage_amount: 0, kill_count: 0, crit_count: 0,
            guard_break_count: 0, additional_moxie: 0, extra_skill_kind: 0, mode,
            teammate_injury_count: 0, teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0, card_enchants: vec![], buff_additions: vec![],
        })
    }

    #[test]
    fn ally_cast_spends_the_buff_source_eureka_once() {
        let manager = fixture(10, 100);
        let catalog = SkillEffectCatalog::default();
        let ops = rule_ops(&manager, &subscriber(), &event(SkillExecutionMode::Active, 11), &catalog).unwrap();
        assert!(matches!(&ops[..], [RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(
            EurekaChange { source_uid: 11, target_uid: 10, power_id: 1, delta: -2, .. }
        )))]));
        assert!(rule_ops(&manager, &subscriber(), &event(SkillExecutionMode::Nested, 11), &catalog).unwrap().is_empty());
        assert!(rule_ops(&manager, &subscriber(), &event(SkillExecutionMode::DirectBig, 11), &catalog).unwrap().is_empty());
        assert!(rule_ops(&manager, &subscriber(), &event(SkillExecutionMode::Active, 10), &catalog).unwrap().is_empty());
        assert!(rule_ops(&fixture(0, 100), &subscriber(), &event(SkillExecutionMode::Active, 11), &catalog).unwrap().is_empty());
        assert!(rule_ops(&fixture(10, 0), &subscriber(), &event(SkillExecutionMode::Active, 11), &catalog).unwrap().is_empty());
        assert!(!supports(&[3, 4]));
    }

    #[test]
    fn ally_cast_adds_current_round_inspiration() {
        let mut manager = fixture(10, 100);
        let key = impromptu::inspiration_key(crate::engine::manager::emitter::UID);
        manager.execute_gauge(GaugeCommand::new(
            CommandOrigin { domain: RuleDomain::Lifecycle, key: DefinitionKey::new(0, "Test") },
            key, GaugeOperation::Enable { max: None },
        )).unwrap();
        let ops = rule_ops(&manager, &subscriber(), &event(SkillExecutionMode::Active, 11), &SkillEffectCatalog::default()).unwrap();
        assert!(matches!(&ops[..], [RuleOp::Command(BattleCommand::Eureka(_)), RuleOp::Command(BattleCommand::Gauge(_))]));
        let RuleOp::Command(BattleCommand::Gauge(command)) = &ops[1] else { unreachable!() };
        assert_eq!(command.key, key);
        assert_eq!(command.operation, GaugeOperation::ChangeValue { delta: 2 });
    }

    #[test]
    fn automatic_or_queued_cast_does_not_spend_eureka_even_with_the_same_skill() {
        let managers = fixture(10, 100);
        let catalog = SkillEffectCatalog::default();
        // A real hand cast of this skill is already recorded. Automatic and queued
        // uses must still be excluded by their own provenance.
        for index in [0, 2] {
            let mut automatic = event(SkillExecutionMode::Active, 11);
            let BattleEvent::SkillAction(action) = &mut automatic else { unreachable!() };
            action.card_index = index;
            assert!(rule_ops(&managers, &subscriber(), &automatic, &catalog).unwrap().is_empty());
        }
        assert_eq!(managers.eureka.get(10, EUREKA_RESOURCE_ID).current, 10);
    }
}
