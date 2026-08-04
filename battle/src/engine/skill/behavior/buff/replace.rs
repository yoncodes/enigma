use super::*;

pub(super) fn consume_buff_command(
    target_uid: i64,
    behavior: &ParsedBehavior,
) -> Option<BuffCommand> {
    let [type_or_buff_id, amount] = behavior.args.as_slice() else {
        return None;
    };
    let selector = match behavior.spec.kind {
        BehaviorKind::ConsumeBuffByTypeId => BuffSelector::IdOrType(*type_or_buff_id),
        BehaviorKind::ConsumeBuffByTypeId2 => BuffSelector::TypeId(*type_or_buff_id),
        _ => return None,
    };
    Some(BuffCommand::Consume(BuffConsume {
        origin: super::command_origin(behavior)?,
        target_uid,
        selector,
        amount: *amount,
        depleted: DepletedBuff::Remove,
    }))
}

pub(super) fn change_duration_command(
    target_uid: i64,
    behavior: &ParsedBehavior,
    selector: fn(i32) -> BuffSelector,
) -> Option<BuffCommand> {
    let [buff_id, delta] = behavior.args.as_slice() else {
        return None;
    };
    Some(BuffCommand::ChangeDuration(BuffChangeDuration {
        origin: super::command_origin(behavior)?,
        target_uid,
        selector: selector(*buff_id),
        delta: *delta,
    }))
}

pub(super) fn replace_buff2_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let origin = super::command_origin(behavior)?;
    let source_buff_ids = behavior.arg_list(0)?;
    let replacement_buff_id = behavior.arg(1)?;
    let threshold = behavior.arg(2)?;
    let limit = behavior.arg(3)?;
    if threshold <= 0 || limit <= 0 || replacement_buff_id <= 0 || source_buff_ids.is_empty() {
        return Some(Vec::new());
    }
    let count = source_buff_ids
        .iter()
        .map(|buff_id| {
            context
                .managers
                .buff
                .buff_id_amount(context.target_uid, *buff_id)
        })
        .sum::<i32>()
        / threshold;
    let count = count.min(limit);
    if count <= 0 {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: context.source_uid,
            target_uid: context.target_uid,
            buff_id: replacement_buff_id,
            amount: Some(count),
            occurrences: 1,
            child_uid_reservations: 0,
        }),
    ))])
}

pub(super) fn consume_power_add_buff_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let cost = behavior.arg(0)?;
    let buff_ids = behavior.arg_list(1)?;
    if cost <= 0
        || buff_ids.is_empty()
        || context
            .managers
            .eureka
            .get(context.source_uid, EUREKA_RESOURCE_ID)
            .current
            < cost
    {
        return Some(Vec::new());
    }
    let origin = super::command_origin(behavior)?;
    let mut ops = Vec::with_capacity(buff_ids.len() + 1);
    ops.push(RuleOp::Command(BattleCommand::Eureka(
        EurekaCommand::Change(EurekaChange {
            origin,
            source_uid: context.source_uid,
            target_uid: context.source_uid,
            power_id: EUREKA_RESOURCE_ID,
            delta: -cost,
            effect_type: sonettobuf::effect_type_enum::EffectType::Powerchange as i32,
        }),
    )));
    ops.extend(
        buff_ids
            .into_iter()
            .filter(|buff_id| *buff_id > 0)
            .map(|buff_id| {
                RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantUsingChildUid(
                    BuffGrant {
                        origin,
                        source_uid: context.source_uid,
                        target_uid: context.target_uid,
                        buff_id,
                        amount: None,
                        occurrences: 1,
                        child_uid_reservations: 0,
                    },
                )))
            }),
    );
    Some(ops)
}

pub(super) fn replace_buff_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [
        counter_buff_id,
        threshold,
        source_buff_id,
        replacement_buff_id,
    ] = behavior.args.as_slice()
    else {
        return Some(Vec::new());
    };
    if *threshold <= 0
        || *source_buff_id <= 0
        || *replacement_buff_id <= 0
        || !context
            .managers
            .buff
            .has_buff_id_or_type(context.target_uid, *source_buff_id)
        || context
            .managers
            .buff
            .max_id_or_type_layer(context.target_uid, *counter_buff_id)
            < *threshold
    {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Replace(BuffReplace {
            origin: super::command_origin(behavior)?,
            source_uid: context.source_uid,
            target_uid: context.target_uid,
            source: BuffSelector::IdOrType(*source_buff_id),
            replacement_id_or_type: *replacement_buff_id,
        }),
    ))])
}

pub(super) fn remove_buff_to_add_buff_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [source_buff_id, replacement_buff_id] = behavior.args.as_slice() else {
        return None;
    };
    if !context
        .managers
        .buff
        .has_buff_id_or_type(context.target_uid, *source_buff_id)
    {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Replace(BuffReplace {
            origin: super::command_origin(behavior)?,
            source_uid: context.source_uid,
            target_uid: context.target_uid,
            source: BuffSelector::IdOrType(*source_buff_id),
            replacement_id_or_type: *replacement_buff_id,
        }),
    ))])
}

pub(super) fn consume_power_add_multi_buff_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [
        cost,
        required_allies,
        required_buff,
        base_layer,
        bonus_layer,
        base_buff,
        bonus_buff,
    ] = behavior.args.as_slice()
    else {
        return Some(Vec::new());
    };
    if *cost <= 0
        || *base_buff <= 0
        || context
            .managers
            .eureka
            .get(context.source_uid, EUREKA_RESOURCE_ID)
            .current
            < *cost
    {
        return Some(Vec::new());
    }
    let origin = super::command_origin(behavior)?;
    let grant = |buff_id, amount| {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantUsingChildUid(
            BuffGrant {
                origin,
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                buff_id,
                amount: Some(amount),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        )))
    };
    let mut ops = vec![
        RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
            origin,
            source_uid: context.source_uid,
            target_uid: context.source_uid,
            power_id: EUREKA_RESOURCE_ID,
            delta: -*cost,
            effect_type: sonettobuf::effect_type_enum::EffectType::Powerchange as i32,
        }))),
        grant(*base_buff, *base_layer),
    ];
    let qualifies = context
        .managers
        .buff
        .alive_team_uids(context.source_team, &context.managers.hp)
        .into_iter()
        .filter(|uid| {
            context
                .managers
                .buff
                .has_active_buff_id_or_type(*uid, *required_buff)
        })
        .count()
        >= (*required_allies).max(0) as usize;
    if qualifies && *bonus_buff > 0 {
        ops.push(grant(*bonus_buff, *bonus_layer));
    }
    Some(ops)
}
