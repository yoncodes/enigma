use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffAccumulateActValue, BuffCommand},
        card::{CardAddGenerated, CardAddTemporary, CardCommand, TemporaryCardKind},
        ex_point::{ExPointCommand, ExPointSet},
    },
    skill::{
        buff_act::registry::BuffActKind,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::AdrenalineAddCard)
        || !matches!(event, BattleEvent::Kind(EventKind::RoundStartCard))
    {
        return None;
    }
    let (thresholds, skill_ids) = parse(&subscriber.args)?;
    let progress = usize::try_from(
        managers
            .buff
            .act_value(subscriber.buff_uid, subscriber.key.definition.opcode),
    )
    .ok()?;
    thresholds.get(progress).zip(skill_ids.get(progress))?;
    let adrenaline = managers.ex_point.get(subscriber.owner_uid);
    let origin = super::command_origin(subscriber)?;
    let hero_id = managers.entity.model_id(subscriber.owner_uid)?;
    let mut ops = Vec::with_capacity((thresholds.len() - progress) * 2);
    for (index, (&threshold, &skill_id)) in
        thresholds.iter().zip(skill_ids).enumerate().skip(progress)
    {
        if adrenaline < threshold {
            break;
        }
        let terminal = index + 1 == thresholds.len();
        if terminal {
            ops.push(RuleOp::Command(BattleCommand::ExPoint(
                ExPointCommand::Set(ExPointSet {
                    origin,
                    source_uid: subscriber.owner_uid,
                    target_uid: subscriber.owner_uid,
                    value: 0,
                    config_effect: 0,
                    effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
                }),
            )));
        }
        if index > 0 || !terminal {
            ops.push(RuleOp::Command(BattleCommand::Buff(
                BuffCommand::AccumulateActValue(BuffAccumulateActValue {
                    origin,
                    target_uid: subscriber.owner_uid,
                    buff_uid: subscriber.buff_uid,
                    act_id: subscriber.key.definition.opcode,
                    delta: if terminal { -(index as i32) } else { 1 },
                }),
            )));
        }
        ops.push(RuleOp::Command(BattleCommand::Card(if terminal {
            CardCommand::AddGenerated(CardAddGenerated {
                origin,
                target_uid: subscriber.owner_uid,
                skill_id,
                hero_id: Some(hero_id),
                team_type: subscriber.team_type,
            })
        } else {
            CardCommand::AddTemporary(CardAddTemporary {
                origin,
                target_uid: subscriber.owner_uid,
                skill_id,
                hero_id: Some(hero_id),
                reserve_id: 0,
                team_type: subscriber.team_type,
                kind: TemporaryCardKind::ConfiguredSkill3,
            })
        })));
    }
    Some(ops)
}

pub fn supports(args: &[i32]) -> bool {
    parse(args).is_some()
}

fn parse(args: &[i32]) -> Option<(&[i32], &[i32])> {
    if args.len() < 2 || !args.len().is_multiple_of(2) {
        return None;
    }
    let (thresholds, skill_ids) = args.split_at(args.len() / 2);
    (thresholds.iter().all(|threshold| *threshold > 0)
        && thresholds.windows(2).all(|pair| pair[0] < pair[1])
        && skill_ids.iter().all(|skill_id| *skill_id > 0))
    .then_some((thresholds, skill_ids))
}

#[cfg(test)]
mod tests;
