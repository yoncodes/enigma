use crate::engine::{
    event::payload::BattleEvent,
    manager::{BattleManagers, buff::ActiveBuffFeature},
    mechanic::impromptu,
    skill::{
        buff_act::{BuffActRuleOp, registry::BuffActKind},
        rule::{SetupStage, output::RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn setup_rule_ops(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    stage: SetupStage,
) -> Option<Vec<RuleOp>> {
    if stage != SetupStage::BattleStart
        || !super::is_kind(feature, BuffActKind::EmitterTag)
        || !super::is_primary_team_feature(managers, feature, BuffActKind::EmitterTag)
    {
        return Some(Vec::new());
    }
    let enable = impromptu::enable_rule_ops(
        managers.catalog().impromptu_definition(),
        &managers.gauge,
        &managers.buff.active_features(&managers.hp),
        crate::engine::manager::emitter::UID,
    )
    .into_iter()
    .find(|enable| enable.team == feature.team_type)?;
    Some(vec![enable.emitter, enable.team_energy, enable.inspiration])
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<BuffActRuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::EmitterTag)
        || !super::is_primary_team_subscriber(managers, subscriber, BuffActKind::EmitterTag)
    {
        return Some(Vec::new());
    }
    let ops = match event {
        BattleEvent::ActionQueueCommitted {
            team,
            emitter_uid,
            cards,
        } if *team == subscriber.team_type => {
            impromptu::action_queue_committed_rule_ops(managers, *team, *emitter_uid, cards)
                .into_iter()
                .map(BuffActRuleOp::separate_independent_command)
                .collect()
        }
        BattleEvent::ImpromptuResolved {
            team, emitter_uid, ..
        } if *team == subscriber.team_type => {
            impromptu::resolved_rule_ops(managers, *team, *emitter_uid)
                .into_iter()
                .map(BuffActRuleOp::causing)
                .collect()
        }
        _ => Vec::new(),
    };
    Some(ops)
}

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let BattleEvent::EurekaChanged(change) = event else {
        return Vec::new();
    };
    if change.applied_delta >= 0 {
        return Vec::new();
    }
    let team = managers.buff.team_type(change.target_uid);
    let Some(feature) = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            Some(feature.team_type) == team && super::is_kind(feature, BuffActKind::EmitterTag)
        })
        .min_by_key(|feature| (feature.owner_uid, feature.buff_uid))
    else {
        return Vec::new();
    };
    impromptu::eureka_spent_rule_op(
        managers,
        change.target_uid,
        change.applied_delta.saturating_abs(),
    )
    .map(|op| vec![(feature, op)])
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        event::payload::EurekaChangeEvent,
        manager::gauge::{GaugeCommand, GaugeOperation},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain, output::BattleCommand},
    };
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    #[test]
    fn eureka_spend_transfers_energy_as_a_buff_transaction() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(1),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(2_240_000),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let origin = super::super::configured_command_origin(875, BuffActKind::EmitterTag).unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                impromptu::team_energy_key(1),
                GaugeOperation::Enable { max: None },
            ))
            .unwrap();
        let event = BattleEvent::EurekaChanged(EurekaChangeEvent {
            origin: CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: DefinitionKey::new(863, "CreateAdditionalDamage"),
            },
            source_uid: 10,
            target_uid: 10,
            power_id: 1,
            before: 4,
            requested_delta: -2,
            applied_delta: -2,
            after: 2,
            overflow: 0,
        });

        let definition =
            crate::engine::skill::buff_act::registry::transaction_definitions(event.kind())
                .find(|definition| definition.key == DefinitionKey::new(875, "EmitterTag"))
                .expect("EmitterTag owns the Eureka transaction route");
        let ops = definition.transaction.handler.unwrap()(&managers, &event);
        let [(feature, RuleOp::Command(BattleCommand::Gauge(command)))] = ops.as_slice() else {
            panic!("expected one team-energy transaction")
        };
        assert_eq!(feature.buff_id, 2_240_000);
        assert_eq!(command.operation, GaugeOperation::ChangeValue { delta: 2 });
    }
}
