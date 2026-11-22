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
mod tests;
