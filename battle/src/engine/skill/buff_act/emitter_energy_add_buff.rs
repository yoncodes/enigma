use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrantChild},
    },
    mechanic::impromptu,
    skill::{
        buff_act::{BuffActRuleOp, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [required_energy, buff_id] if *required_energy > 0 && *buff_id > 0)
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<BuffActRuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::EmitterEnergyAddBuff) {
        return Some(Vec::new());
    }
    let BattleEvent::PlayerActionsResolved { team, emitter_uid } = event else {
        return Some(Vec::new());
    };
    if *team != subscriber.team_type || *emitter_uid != subscriber.owner_uid {
        return Some(Vec::new());
    }
    let thresholds = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == subscriber.owner_uid
                && feature.buff_uid == subscriber.buff_uid
                && super::is_kind(feature, BuffActKind::EmitterEnergyAddBuff)
        })
        .filter_map(|feature| {
            let [_, required_energy, buff_id, ..] = feature.values.as_slice() else {
                return None;
            };
            Some((*required_energy, *buff_id))
        })
        .collect::<Vec<_>>();
    let (first_required, first_buff) = thresholds.first()?;
    if subscriber.args.as_slice() != [*first_required, *first_buff] {
        return Some(Vec::new());
    }
    let energy = managers
        .gauge
        .get(impromptu::inspiration_key(*emitter_uid))
        .map(|state| state.current)
        .unwrap_or_default();
    let origin = super::command_origin(subscriber)?;
    Some(
        thresholds
            .into_iter()
            .filter(|(required_energy, buff_id)| {
                energy >= *required_energy && !managers.buff.has_buff_id(*emitter_uid, *buff_id)
            })
            .map(|(_, buff_id)| {
                BuffActRuleOp::independent_event(RuleOp::Command(BattleCommand::Buff(
                    BuffCommand::GrantChild(BuffGrantChild {
                        origin,
                        source_uid: *emitter_uid,
                        target_uid: *emitter_uid,
                        buff_id,
                        amount: Some(0),
                        params: None,
                        act_info: None,
                    }),
                )))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        event::kind::EventKind,
        manager::{BattleManagers, emitter, gauge::GaugeCommand},
        skill::{buff_act, rule::output::BattleCommand},
    };

    #[test]
    fn inspiration_thresholds_activate_visible_attack_buffs() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(emitter::UID),
                    team_type: Some(1),
                    current_hp: Some(1),
                    buffs: vec![BuffInfo {
                        uid: Some(30),
                        buff_id: Some(31080151),
                        from_uid: Some(emitter::UID),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let subscriber = crate::engine::skill::subscriber::for_active_buffs(
            &managers,
            EventKind::PlayerActionsResolved,
        )
        .into_iter()
        .find(|subscriber| subscriber.args.first() == Some(&3))
        .unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                buff_act::command_origin(&subscriber).unwrap(),
                impromptu::inspiration_key(emitter::UID),
                crate::engine::manager::gauge::GaugeOperation::Enable { max: None },
            ))
            .unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                buff_act::command_origin(&subscriber).unwrap(),
                impromptu::inspiration_key(emitter::UID),
                crate::engine::manager::gauge::GaugeOperation::ChangeValue { delta: 20 },
            ))
            .unwrap();

        let event = BattleEvent::PlayerActionsResolved {
            team: 1,
            emitter_uid: emitter::UID,
        };
        let ops = rule_ops(&managers, &subscriber, &event).unwrap();
        assert_eq!(ops.len(), 4);
        for op in ops {
            let RuleOp::Command(BattleCommand::Buff(command)) = op.op else {
                panic!("expected a buff command")
            };
            let changes = managers.execute_buff(command).unwrap();
            assert!(changes.is_wire_visible());
            assert!(!changes.events().is_empty());
        }
        for buff_id in 31080152..=31080155 {
            assert!(managers.buff.has_buff_id(emitter::UID, buff_id));
        }
    }
}
