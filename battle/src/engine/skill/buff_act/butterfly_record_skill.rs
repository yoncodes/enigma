use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffActInfoMarkerResult, BuffCommand, BuffSetState},
        card::{CardAddPrecast, CardCommand, selected_precast_card},
    },
    skill::{
        action::SkillRequest,
        behavior::{BehaviorOpContext, classify::BehaviorKind},
        effect::SkillEffectCatalog,
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
    let Some(feature) = record_feature(context.managers, context.source_uid).filter(|feature| {
        allows_recorded_skill(context.managers.catalog(), feature, skill.skill_id)
    }) else {
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

fn record_feature(managers: &BattleManagers, owner_uid: i64) -> Option<ActiveBuffFeature> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| {
            feature.owner_uid == owner_uid
                && super::is_kind(feature, super::registry::BuffActKind::ButterflyRecordSkill)
        })
}

pub(crate) fn auto_record(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    hand: &[sonettobuf::CardInfo],
    cast: SkillRequest,
) -> Option<SkillRequest> {
    let effect = catalog.get(cast.skill_id)?;
    if !effect
        .slots
        .iter()
        .any(|slot| slot.behavior.spec.kind == BehaviorKind::BufferflyRecordSkill)
    {
        return None;
    }
    let feature = record_feature(managers, cast.source_uid)?;
    let data = managers.catalog();
    let (_, skill_id) = hand
        .iter()
        .filter_map(|card| {
            let skill_id = card.skill_id?;
            (card.uid == Some(cast.source_uid) && allows_recorded_skill(data, &feature, skill_id))
                .then_some((card, skill_id))
        })
        .min_by_key(|(_, skill_id)| catalog.damage_rate(*skill_id) <= 0)?;
    Some(SkillRequest {
        source_uid: cast.source_uid,
        skill_id,
    })
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
    let Some(caster) = pool
        .allies(subscriber.owner_uid)
        .iter()
        .find(|entity| entity.uid == skill.source_uid)
    else {
        return Some(Vec::new());
    };
    let enchant_id = *subscriber.args.get(1)?;
    let origin = super::command_origin(subscriber)?;
    let mut card = selected_precast_card(caster.uid, caster.model_id, skill.skill_id);
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
