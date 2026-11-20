use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffActInfoMarkerResult, BuffCommand, BuffSetState},
        card::{CardAddPrecast, CardCommand, temp::runtime_precast_card},
    },
    skill::{
        action::SkillRequest,
        behavior::BehaviorOpContext,
        rule::{
            CommandOrigin,
            output::{BattleCommand, RuleOp},
        },
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub(crate) fn record(context: BehaviorOpContext<'_>, origin: CommandOrigin) -> Option<Vec<RuleOp>> {
    let skill = SkillRequest {
        source_uid: context.target.recorded_skill_source_uid,
        skill_id: context.target.recorded_skill_id,
    };
    if skill.skill_id <= 0 || skill.source_uid == 0 {
        return Some(Vec::new());
    }
    let Some(feature) = context
        .managers
        .buff
        .active_features(&context.managers.hp)
        .into_iter()
        .find(|feature| {
            feature.owner_uid == context.source_uid
                && super::is_kind(feature, super::registry::BuffActKind::ButterflyRecordSkill)
        })
        .filter(|feature| {
            allows_recorded_skill(context.managers.catalog(), feature, skill.skill_id)
        })
    else {
        return Some(Vec::new());
    };
    let &[act_id, count, _, ref allowed @ ..] = feature.values.as_slice() else {
        return None;
    };
    let mut buff = context
        .managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)?;
    let info = buff
        .act_info
        .iter_mut()
        .find(|info| info.act_id == Some(act_id))?;
    let record = format!("{count},{},{}", skill.skill_id, skill.source_uid);
    info.param = allowed.to_vec();
    info.str_param = Some(record.clone());
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
            params: allowed.to_vec(),
            str_param: Some(record),
            team_type: 0,
        }),
    ])
}

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !matches!(
        event,
        BattleEvent::Kind(crate::engine::event::kind::EventKind::RoundStartCard)
    ) {
        return None;
    }
    let act_id = subscriber.key.definition.opcode;
    let Some(mut buff) = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)
    else {
        return Some(Vec::new());
    };
    let Some(info) = buff
        .act_info
        .iter_mut()
        .find(|info| info.act_id == Some(act_id))
    else {
        return Some(Vec::new());
    };
    let Some((count, skill)) = parse_record(info) else {
        return Some(Vec::new());
    };
    if !pool
        .allies(subscriber.owner_uid)
        .iter()
        .any(|entity| entity.uid == skill.source_uid)
    {
        return Some(Vec::new());
    }
    let enchant_id = *subscriber.args.get(1)?;
    let origin = super::command_origin(subscriber)?;
    let mut card = runtime_precast_card(managers, skill.source_uid, skill.skill_id);
    card.enchants.push(sonettobuf::CardEnchant {
        enchant_id: Some(enchant_id),
        duration: Some(1),
        ex_info: Vec::new(),
    });
    let mut ops = (0..count)
        .map(|_| {
            RuleOp::Command(BattleCommand::Card(CardCommand::AddPrecast(
                CardAddPrecast {
                    origin,
                    card: card.clone(),
                },
            )))
        })
        .collect::<Vec<_>>();
    info.str_param = Some(format!("{count},0,0"));
    ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(
        BuffSetState {
            origin,
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            ex_info: None,
            params: None,
            act_info: Some(buff.act_info),
        },
    ))));
    Some(ops)
}

fn parse_record(info: &sonettobuf::BuffActInfo) -> Option<(i32, SkillRequest)> {
    let mut values = info.str_param.as_deref()?.split(',');
    let count = values.next()?.parse::<i32>().ok()?;
    let skill_id = values.next()?.parse::<i32>().ok()?;
    let source_uid = values.next()?.parse::<i64>().ok()?;
    (count > 0 && skill_id > 0 && source_uid != 0 && values.next().is_none()).then_some((
        count,
        SkillRequest {
            source_uid,
            skill_id,
        },
    ))
}

fn allows_recorded_skill(
    catalog: crate::catalog::BattleCatalog,
    feature: &ActiveBuffFeature,
    skill_id: i32,
) -> bool {
    if catalog.skill_is_big(skill_id) {
        return false;
    }
    let effect_tag = catalog.skill_effect_tag(skill_id);
    feature
        .values
        .get(3..)
        .is_some_and(|allowed| allowed.contains(&effect_tag))
}

#[cfg(test)]
mod tests;
