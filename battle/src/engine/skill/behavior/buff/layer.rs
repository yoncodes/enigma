use super::*;

pub(super) fn multiply_buff_count_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [buff_id, multiplier] = behavior.args.as_slice() else {
        return None;
    };
    let additions = context
        .managers
        .buff
        .buff_id_amount(context.target_uid, *buff_id)
        .saturating_mul(multiplier.saturating_sub(1));
    let origin = super::command_origin(behavior)?;
    Some(
        (0..additions)
            .map(|_| {
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Accumulate(BuffGrant {
                    origin,
                    source_uid: context.source_uid,
                    target_uid: context.target_uid,
                    buff_id: *buff_id,
                    amount: None,
                    occurrences: 1,
                    child_uid_reservations: 0,
                })))
            })
            .collect(),
    )
}

pub(super) fn add_buff_by_layer_ops(
    context: &mut BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [source_buff_id, output_buff_id, multiplier] = behavior.args.as_slice() else {
        return None;
    };
    if *source_buff_id <= 0 || *output_buff_id <= 0 || *multiplier <= 0 {
        return None;
    }
    let amount = context
        .managers
        .buff
        .buff_id_or_type_amount(context.source_uid, *source_buff_id)
        .saturating_mul(*multiplier);
    if amount <= 0 {
        return Some(Vec::new());
    }
    if context.target_uid == context.target.runtime_target_uid {
        context.target.buff_overflow_amount = context.managers.buff.grant_overflow(
            context.source_uid,
            context.target_uid,
            *output_buff_id,
            amount,
        );
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Grant(BuffGrant {
            origin: super::command_origin(behavior)?,
            source_uid: context.source_uid,
            target_uid: context.target_uid,
            buff_id: *output_buff_id,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        }),
    ))])
}

pub(super) fn add_buff_by_layer_range_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let source_buff_id = behavior.arg(0)?;
    let buff_ids = behavior.arg_list(1)?;
    let thresholds = behavior.arg_list(2)?;
    let layer = context
        .managers
        .buff
        .buff_id_or_type_amount(context.source_uid, source_buff_id);
    let Some(selected) = buff_ids
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, buff_id)| {
            (layer >= thresholds.get(index).copied().unwrap_or(i32::MAX)).then_some(*buff_id)
        })
    else {
        return Some(Vec::new());
    };
    let origin = super::command_origin(behavior)?;
    let mut ops = buff_ids
        .into_iter()
        .map(|buff_id| {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Deactivate(BuffRemove {
                origin,
                target_uid: context.target_uid,
                selector: BuffRemoveSelector::ExactId(buff_id),
            })))
        })
        .collect::<Vec<_>>();
    ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
        BuffGrant {
            origin,
            source_uid: context.source_uid,
            target_uid: context.target_uid,
            buff_id: selected,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        },
    ))));
    Some(ops)
}

pub(super) fn consume_buff_layer_and_team_grant_ops(
    context: &mut BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [source_target, source_buff_id, max_amount, output_buff_id] = behavior.args.as_slice()
    else {
        return None;
    };
    if *source_buff_id <= 0 || *max_amount <= 0 || *output_buff_id <= 0 {
        return None;
    }
    let owner_uid = TargetResolver::resolve_with_context(
        &TargetRequest {
            code: *source_target,
            raw: Vec::new(),
        },
        context.active_skill_id,
        context.source_uid,
        context.pool,
        context.determinism,
        *context.target,
    )
    .into_iter()
    .next()?;
    let amount = context
        .managers
        .buff
        .buff_id_or_type_amount(owner_uid, *source_buff_id)
        .min(*max_amount);
    if amount <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(behavior)?;
    let mut ops = vec![RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(
        BuffConsume {
            origin,
            target_uid: owner_uid,
            selector: BuffSelector::IdOrType(*source_buff_id),
            amount,
            depleted: DepletedBuff::Remove,
        },
    )))];
    ops.extend(context.pool.allies(context.source_uid).iter().map(|ally| {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: context.source_uid,
            target_uid: ally.uid,
            buff_id: *output_buff_id,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        })))
    }));
    Some(ops)
}

pub(super) fn add_buff_from_enemy_burn_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [buff_id, rate] = behavior.args.as_slice() else {
        return Some(Vec::new());
    };
    let consumed = match context.event {
        Some(crate::engine::event::payload::BattleEvent::BuffsSettled(changes)) => changes
            .iter()
            .filter(|change| {
                context.managers.buff.team_type(change.target_uid) != Some(context.source_team)
                    && crate::engine::manager::buff::BuffManager::configured_features(
                        change.buff_id,
                    )
                    .iter()
                    .any(|feature| is_kind(feature, BuffActKind::Burn))
            })
            .map(|change| change.before_amount - change.after_amount)
            .max()
            .unwrap_or_default(),
        _ => 0,
    };
    let amount = consumed * (*rate).max(0) / 1000;
    if *buff_id <= 0 || amount <= 0 {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Accumulate(BuffGrant {
            origin: super::command_origin(behavior)?,
            source_uid: context.source_uid,
            target_uid: context.target_uid,
            buff_id: *buff_id,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        }),
    ))])
}
