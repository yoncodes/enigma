use super::*;

pub(super) fn spread_buff_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [buff_id, rate] = behavior.args.as_slice() else {
        return None;
    };
    let amount = context
        .managers
        .buff
        .buff_id_or_type_amount(context.target_uid, *buff_id)
        .saturating_mul(*rate)
        / 1000;
    if *buff_id <= 0 || *rate <= 0 || amount <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(behavior)?;
    Some(
        context
            .pool
            .enemies(context.source_uid, true)
            .iter()
            .filter(|target| {
                target.uid != context.target_uid && context.managers.hp.current(target.uid) > 0
            })
            .map(|target| {
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                    origin,
                    source_uid: context.source_uid,
                    target_uid: target.uid,
                    buff_id: *buff_id,
                    amount: Some(amount),
                    occurrences: 1,
                    child_uid_reservations: 0,
                })))
            })
            .collect(),
    )
}

pub(super) fn sort_buff_by_hp_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let buff_id = behavior.arg(0)?;
    let stack_limit = context.managers.buff.stack_limit(buff_id);
    if buff_id <= 0 || stack_limit <= 0 {
        return Some(Vec::new());
    }

    let mut enemies = context
        .pool
        .enemies(context.source_uid, true)
        .iter()
        .filter(|enemy| context.managers.hp.current(enemy.uid) > 0)
        .map(|enemy| (enemy.uid, enemy.position))
        .collect::<Vec<_>>();
    enemies.sort_by_key(|(uid, position)| {
        (
            std::cmp::Reverse(context.managers.hp.current(*uid)),
            *position,
            *uid,
        )
    });

    let mut remaining = enemies
        .iter()
        .map(|(uid, _)| context.managers.buff.buff_id_amount(*uid, buff_id))
        .sum::<i32>();
    let origin = super::command_origin(behavior)?;
    let mut ops = Vec::new();
    for (target_uid, _) in enemies {
        let current = context.managers.buff.buff_id_amount(target_uid, buff_id);
        let assigned = remaining.min(stack_limit).max(0);
        remaining -= assigned;
        if assigned == current {
            continue;
        }

        let buff_uid = context
            .managers
            .buff
            .active_for(target_uid)
            .find(|buff| buff.buff_id == Some(buff_id))
            .and_then(|buff| buff.uid);
        let command = match (buff_uid, assigned) {
            (Some(buff_uid), 0) => BuffCommand::Remove(BuffRemove {
                origin,
                target_uid,
                selector: BuffRemoveSelector::Uid(buff_uid),
            }),
            (Some(buff_uid), assigned) => BuffCommand::SetAmount(BuffSetAmount {
                origin,
                target_uid,
                buff_uid,
                amount: BuffAmount::Layer(assigned),
            }),
            (None, assigned) if assigned > 0 => BuffCommand::Grant(BuffGrant {
                origin,
                source_uid: context.source_uid,
                target_uid,
                buff_id,
                amount: Some(assigned),
                occurrences: 1,
                child_uid_reservations: 0,
            }),
            (None, _) => continue,
        };
        ops.push(RuleOp::Command(BattleCommand::Buff(command)));
    }
    Some(ops)
}

pub(super) fn damage_window_remove_ops(
    target_uid: i64,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let origin = super::command_origin(behavior)?;
    Some(
        behavior
            .arg_list(0)?
            .into_iter()
            .filter(|buff_id| *buff_id > 0)
            .map(|buff_id| {
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                    origin,
                    target_uid,
                    selector: BuffRemoveSelector::IdOrType(buff_id),
                })))
            })
            .collect(),
    )
}

pub(in crate::engine::skill::behavior) fn supports_disperse_force(
    behavior: &ParsedBehavior,
) -> bool {
    behavior
        .arg_list(0)
        .is_some_and(|ids| !ids.is_empty() && ids.into_iter().all(|id| id > 0))
}

pub(in crate::engine::skill::behavior) fn supports_exact_buff_dispel(
    behavior: &ParsedBehavior,
) -> bool {
    !behavior.args.is_empty() && behavior.args.iter().all(|buff_id| *buff_id > 0)
}

pub(super) fn dispel_commands(
    target_uid: i64,
    behavior: &ParsedBehavior,
) -> Option<Vec<BuffCommand>> {
    let (count, first_status) = if behavior.spec.kind == BehaviorKind::PurifyX {
        (behavior.arg(0)?, 1)
    } else {
        (0, 0)
    };
    let status_args = (first_status..behavior.raw_args.len().max(behavior.args.len()))
        .map(|index| behavior.arg_list(index))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let statuses = status_args
        .iter()
        .copied()
        .map(BuffStatus::from_id)
        .filter(|status| *status != BuffStatus::Unknown)
        .collect::<Vec<_>>();
    if !statuses.is_empty() && statuses.len() == status_args.len() {
        return Some(vec![BuffCommand::Dispel(BuffDispel {
            origin: super::command_origin(behavior).expect("registered dispel behavior"),
            target_uid,
            statuses,
            excluded_ids_or_types: Vec::new(),
            count,
        })]);
    }
    if matches!(
        behavior.spec.kind,
        BehaviorKind::Purify1 | BehaviorKind::PurifyX
    ) {
        return None;
    }
    let origin = super::command_origin(behavior)?;
    let commands = status_args
        .iter()
        .copied()
        .filter(|buff_id| *buff_id > 0)
        .map(|buff_id| {
            BuffCommand::Remove(BuffRemove {
                origin,
                target_uid,
                selector: BuffRemoveSelector::ExactId(buff_id),
            })
        })
        .collect::<Vec<_>>();
    (!commands.is_empty()).then_some(commands)
}

pub(super) fn excluded_dispel_command(
    target_uid: i64,
    behavior: &ParsedBehavior,
) -> Option<BuffCommand> {
    let excluded_ids_or_types = behavior.arg_list(0)?;
    let status_ids = behavior.arg_list(1)?;
    let statuses = status_ids
        .iter()
        .copied()
        .map(BuffStatus::from_id)
        .collect::<Vec<_>>();
    if excluded_ids_or_types.is_empty()
        || excluded_ids_or_types.iter().any(|id| *id <= 0)
        || statuses.is_empty()
        || statuses.contains(&BuffStatus::Unknown)
    {
        return None;
    }
    Some(BuffCommand::Dispel(BuffDispel {
        origin: super::command_origin(behavior)?,
        target_uid,
        statuses,
        excluded_ids_or_types,
        count: 0,
    }))
}

pub(in crate::engine::skill::behavior) fn supports_dispel(behavior: &ParsedBehavior) -> bool {
    dispel_commands(0, behavior).is_some()
}

pub(in crate::engine::skill::behavior) fn supports_excluded_dispel(
    behavior: &ParsedBehavior,
) -> bool {
    excluded_dispel_command(1, behavior).is_some()
}

pub(in crate::engine::skill::behavior) fn supports_status_dispel(
    behavior: &ParsedBehavior,
) -> bool {
    let count = behavior.raw_args.len().max(behavior.args.len());
    count > 0
        && (0..count).all(|index| {
            behavior.arg_list(index).is_some_and(|statuses| {
                !statuses.is_empty()
                    && statuses
                        .into_iter()
                        .all(|status| BuffStatus::from_id(status) != BuffStatus::Unknown)
            })
        })
}
