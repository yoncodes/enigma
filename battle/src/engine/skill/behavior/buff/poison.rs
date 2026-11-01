use super::*;

pub(in crate::engine::skill::behavior) fn supports_add_target_buff_by_poison(
    behavior: &ParsedBehavior,
) -> bool {
    matches!(
        behavior.args.as_slice(),
        [2, 2, buff_id, 2] if *buff_id > 0
    ) && behavior.raw_args.len() <= 4
}

pub(super) fn add_target_buff_by_poison_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    if behavior.spec.kind != BehaviorKind::AddTargetBuffByPoison {
        return None;
    }
    let [instances, duration, buff_id, max_targets] = behavior.args.as_slice() else {
        return None;
    };
    if *instances <= 0 || *duration <= 0 || *buff_id <= 0 || *max_targets <= 0 {
        return None;
    }

    let mut enemies = context
        .pool
        .enemies(context.source_uid, false)
        .iter()
        .filter(|enemy| context.managers.hp.current(enemy.uid) > 0)
        .map(|enemy| {
            (
                enemy.uid,
                enemy.position,
                context
                    .managers
                    .buff
                    .buff_act_amount(enemy.uid, BuffActKind::Poison),
            )
        })
        .collect::<Vec<_>>();
    enemies.sort_by_key(|(uid, position, poison)| (std::cmp::Reverse(*poison), *position, *uid));
    enemies.truncate(*max_targets as usize);
    if enemies.first().is_some_and(|(_, _, poison)| *poison > 0) {
        enemies.truncate(1);
    }

    let origin = super::command_origin(behavior)?;
    let mut ops = Vec::new();
    for (target_uid, _, _) in enemies {
        ops.push(RuleOp::Command(BattleCommand::Buff(
            BuffCommand::ReserveChildUids(BuffChildUidReservation {
                origin,
                target_uid,
                count: 1,
            }),
        )));
        ops.extend((0..*instances).map(|_| {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantChild(
                BuffGrantChild {
                    origin,
                    source_uid: context.source_uid,
                    target_uid,
                    buff_id: *buff_id,
                    amount: None,
                    params: None,
                    act_info: None,
                },
            )))
        }));
    }
    Some(ops)
}
