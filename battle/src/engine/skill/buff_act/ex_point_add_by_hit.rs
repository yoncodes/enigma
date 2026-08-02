use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffAmount, BuffCommand, BuffSetAmount},
        ex_point::{ExPointChange, ExPointCommand},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::ExPointAddByHit) {
        return None;
    }
    let BattleEvent::Hit(hit) = event else {
        return Some(Vec::new());
    };
    if hit.target_uid != subscriber.owner_uid {
        return Some(Vec::new());
    }
    let layer = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?
        .layer
        .unwrap_or_default();
    if layer <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    let delta = subscriber.args.first().copied().unwrap_or(1);
    Some(vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetAmount(BuffSetAmount {
            origin,
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            amount: BuffAmount::Layer(layer - 1),
        }))),
        RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
            ExPointChange {
                origin,
                source_uid: hit.source_uid,
                target_uid: hit.source_uid,
                delta,
                config_effect: 0,
                effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
            },
        ))),
    ])
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use crate::engine::{
        event::{bus::EventBus, kind::EventKind, payload::HitEvent, subscription::SubscriptionKey},
        manager::BattleManagers,
        runtime::executor::execute_rule_op,
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    use super::*;

    #[test]
    fn hit_event_emits_manager_commands_with_exact_origin() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(30620111),
                        from_uid: Some(10),
                        layer: Some(2),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let subscriber = BuffActSubscriber {
            owner_uid: -1,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30620111,
            team_type: 2,
            owner_alive: true,
            amount: 2,
            key: SubscriptionKey::new(
                EventKind::BeAttacked,
                crate::engine::skill::rule::DefinitionKey::new(926, "ExPointAddByHit"),
            ),
            act_type: "ExPointAddByHit".to_owned(),
            effect_time: 209,
            effect_condition: 0,
            args: vec![1],
            raw: "926#1".to_owned(),
        };
        let event = BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "Damage"),
            },
            source_uid: 10,
            target_uid: -1,
            skill_id: 100,
            amount: 50,
            shield_absorbed: 0,
            damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
            assassinate: false,
            ignore_riposte: false,
        });

        assert!(super::super::registry::has_destination(
            926,
            "ExPointAddByHit",
            &[1]
        ));
        let ops = rule_ops(&managers, &subscriber, &event).unwrap();
        assert_eq!(ops.len(), 2);
        let mut events = EventBus::default();
        for op in ops {
            execute_rule_op(&mut managers, &mut events, op).unwrap();
        }

        assert_eq!(managers.buff.snapshot(-1, 20).unwrap().layer, Some(1));
        assert_eq!(managers.ex_point.get(10), 1);
    }
}
