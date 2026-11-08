use sonettobuf::BuffActInfo;

use crate::engine::{
    damage::scale_permille,
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffActInfoMarkerResult, BuffCommand, BuffSetState},
        hedone_counter::{HedoneCounterCommand, HedoneCounterRecord, HedoneCounterSettle},
        hp::HurtDamageFromType,
    },
    skill::{
        buff_act::BuffActRuleOp,
        buff_act::registry::RuntimeContext,
        rule::{
            SetupStage,
            output::{BattleCommand, RuleOp},
        },
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [base, scar_buff_id, extra, retained_rounds]
        if *base > 0 && *scar_buff_id > 0 && *extra >= 0 && *retained_rounds > 0)
}

pub fn setup_rule_ops(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    stage: SetupStage,
) -> Option<Vec<RuleOp>> {
    if stage != SetupStage::EnterBattleStatic
        || !super::is_kind(feature, super::registry::BuffActKind::HedoneHurtCounter)
    {
        return Some(Vec::new());
    }
    let act_id = feature.act_id()?;
    let mut buff = managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)?;
    if buff.act_info.iter().any(|info| info.act_id == Some(act_id)) {
        return Some(Vec::new());
    }
    buff.act_info.push(BuffActInfo {
        act_id: Some(act_id),
        param: vec![0],
        str_param: Some(String::new()),
    });
    let origin = super::feature_command_origin(feature)?;
    Some(vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
            BuffSetState {
                origin,
                target_uid: feature.owner_uid,
                buff_uid: feature.buff_uid,
                ex_info: None,
                params: None,
                act_info: Some(buff.act_info),
            },
        ))),
        RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
            target_uid: feature.owner_uid,
            buff_uid: feature.buff_uid,
            act_id,
            params: vec![0],
            str_param: Some(String::new()),
            team_type: 0,
        }),
    ])
}

fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Result<Vec<RuleOp>, crate::engine::manager::hedone_counter::HedoneCounterError> {
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::HedoneHurtCounter) {
        return Ok(Vec::new());
    }
    let [base, scar_buff_id, extra, retained_rounds] = subscriber.args.as_slice() else {
        return Ok(Vec::new());
    };
    let visible = managers
        .buff
        .act_value(subscriber.buff_uid, subscriber.key.definition.opcode);
    let Some(origin) = super::command_origin(subscriber) else {
        return Ok(Vec::new());
    };
    match event {
        BattleEvent::Hit(hit)
            if pool.team_type(hit.source_uid) == Some(subscriber.team_type)
                && pool.team_type(hit.target_uid) != Some(subscriber.team_type)
                && matches!(
                    hit.damage_from,
                    HurtDamageFromType::Skill | HurtDamageFromType::SkillEffect
                ) =>
        {
            let amount = scale_permille(hit.resolved_damage, *base).saturating_add(
                managers
                    .buff
                    .has_buff_id(hit.target_uid, *scar_buff_id)
                    .then(|| scale_permille(hit.resolved_damage, *extra))
                    .unwrap_or_default(),
            );
            Ok((amount > 0)
                .then(|| {
                    vec![RuleOp::Command(BattleCommand::HedoneCounter(
                        HedoneCounterCommand::Record(HedoneCounterRecord {
                            origin,
                            buff_uid: subscriber.buff_uid,
                            visible,
                            amount,
                        }),
                    ))]
                })
                .unwrap_or_default())
        }
        BattleEvent::Kind(EventKind::RoundEndAfterFinalSettlement) => {
            let settle = HedoneCounterSettle {
                origin,
                buff_uid: subscriber.buff_uid,
                visible,
                retained_rounds: usize::try_from(*retained_rounds).map_err(|_| {
                    crate::engine::manager::hedone_counter::HedoneCounterError::InvalidRetainedRounds(
                        *retained_rounds,
                    )
                })?,
            };
            let value = managers.hedone_counter.settled_value(settle)?;
            Ok(vec![
                RuleOp::Command(BattleCommand::HedoneCounter(HedoneCounterCommand::Settle(
                    settle,
                ))),
                RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
                    BuffSetState {
                        origin,
                        target_uid: subscriber.owner_uid,
                        buff_uid: subscriber.buff_uid,
                        ex_info: None,
                        params: None,
                        act_info: Some(vec![BuffActInfo {
                            act_id: Some(subscriber.key.definition.opcode),
                            param: vec![value],
                            str_param: Some(String::new()),
                        }]),
                    },
                ))),
                RuleOp::BuffActInfoMarker(crate::engine::manager::buff::BuffActInfoMarkerResult {
                    target_uid: subscriber.owner_uid,
                    buff_uid: subscriber.buff_uid,
                    act_id: subscriber.key.definition.opcode,
                    params: vec![value],
                    str_param: Some(String::new()),
                    team_type: 0,
                }),
            ])
        }
        _ => Ok(Vec::new()),
    }
}

pub fn scoped_rule_ops(
    context: &mut RuntimeContext<'_>,
) -> Result<Vec<BuffActRuleOp>, super::BuffActRuleError> {
    Ok(rule_ops(
        context.managers,
        context.pool,
        context.subscriber,
        context.event.expect("runtime handler requires an event"),
    )?
    .into_iter()
    .map(BuffActRuleOp::subscriber)
    .collect())
}

#[cfg(test)]
mod tests;
