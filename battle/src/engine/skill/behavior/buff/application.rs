use super::*;

pub(super) fn hero_grant_command(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<BuffCommand> {
    let definition = super::registry::find(behavior)?;
    if definition.kind != BehaviorKind::AddBuffByHeroId {
        return None;
    }
    let model_id = context.pool.entity(context.target_uid)?.model_id;
    let heroes = behavior.arg_list(0)?;
    let buffs = behavior
        .raw_args
        .iter()
        .skip(1)
        .flat_map(|raw| raw.split(','))
        .filter_map(|raw| raw.trim().parse::<i32>().ok())
        .collect::<Vec<_>>();
    let buff_id = heroes
        .into_iter()
        .zip(buffs)
        .find_map(|(hero_id, buff_id)| (hero_id == model_id && buff_id > 0).then_some(buff_id))?;
    Some(BuffCommand::Grant(BuffGrant {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: definition.key,
        },
        source_uid: context.source_uid,
        target_uid: context.target_uid,
        buff_id,
        amount: None,
        occurrences: context.transfer_count.max(0) as u32,
        child_uid_reservations: 0,
    }))
}

pub(super) fn shield_grant_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    if !matches!(
        behavior.spec.kind,
        BehaviorKind::AddBuff | BehaviorKind::AddBuffRound | BehaviorKind::AddBuffRound2
    ) {
        return None;
    }
    let buff_id = behavior.arg(0)?;
    let (attr, rate) = crate::engine::skill::buff_act::shield::configured_attr_rate(
        buff_id,
        context.source_uid,
        &context.managers.buff,
    )?;
    let origin = super::command_origin(behavior)?;
    Some(
        (0..context.transfer_count.max(0))
            .map(|_| {
                RuleOp::Command(BattleCommand::Shield(ShieldCommand {
                    origin,
                    source_uid: context.source_uid,
                    target_uid: context.target_uid,
                    buff_id,
                    amount_attr: attr,
                    amount_rate: rate,
                    multiplier_bonus: None,
                    max_attr: attr,
                    max_rate: rate,
                    scope: crate::engine::manager::shield::ShieldScope::Entity,
                    carrier_uid: crate::engine::manager::shield::ShieldCarrierUid::Definition,
                }))
            })
            .collect(),
    )
}

pub(super) fn heat_scale_snapshot_grant_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    if !matches!(
        behavior.spec.kind,
        BehaviorKind::AddBuff | BehaviorKind::AddBuffRound | BehaviorKind::AddBuffRound2
    ) {
        return None;
    }
    let buff_id = behavior.arg(0)?;
    let act_info = crate::engine::skill::buff_act::attr_by_heat_scale::snapshot(
        buff_id,
        context.target.heat_scale_value,
    )?;
    let origin = super::command_origin(behavior)?;
    Some(
        (0..context.transfer_count.max(0))
            .map(|_| {
                RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantStateful(
                    crate::engine::manager::buff::BuffGrantChild {
                        origin,
                        source_uid: context.source_uid,
                        target_uid: context.target_uid,
                        buff_id,
                        amount: behavior.arg(1),
                        params: None,
                        act_info: Some(act_info.clone()),
                    },
                )))
            })
            .collect(),
    )
}

pub(super) fn team_energy_snapshot_grant_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    if !matches!(
        behavior.spec.kind,
        BehaviorKind::AddBuff | BehaviorKind::AddBuffRound | BehaviorKind::AddBuffRound2
    ) {
        return None;
    }
    let buff_id = behavior.arg(0)?;
    let team_type = context.managers.entity.team_type(context.target_uid)?;
    let params = crate::engine::skill::buff_act::fix_attr_team_energy::grant_params(
        context.managers,
        context.pool,
        buff_id,
        context.target_uid,
        team_type,
    )?;
    let origin = super::command_origin(behavior)?;
    Some(
        (0..context.transfer_count.max(0))
            .map(|_| {
                RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantStateful(
                    BuffGrantChild {
                        origin,
                        source_uid: context.source_uid,
                        target_uid: context.target_uid,
                        buff_id,
                        amount: behavior.arg(1),
                        params: Some(params.clone()),
                        act_info: None,
                    },
                )))
            })
            .collect(),
    )
}

pub(super) fn consume_card_grant_ops(
    context: &BehaviorOpContext<'_>,
    target_uid: i64,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let definition = super::registry::find(behavior)?;
    if definition.kind != BehaviorKind::ConsumeCardAddBuff {
        return None;
    }
    let rewards = behavior.arg_list(1)?;
    let plan = context
        .managers
        .card
        .plan_effect_consumption(context.source_uid);
    let amount = plan.iter().try_fold(0_i32, |total, (_, rank)| {
        let reward = rewards.get(usize::try_from(rank.checked_sub(1)?).ok()?)?;
        total.checked_add(*reward)
    })?;
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: definition.key,
    };
    let mut ops = vec![RuleOp::Command(BattleCommand::Card(
        CardCommand::ConsumeForEffect(CardConsumeForEffect {
            origin,
            owner_uid: context.source_uid,
            indices: plan.iter().map(|(index, _)| *index).collect(),
        }),
    ))];
    if amount > 0 {
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin,
                source_uid: context.source_uid,
                target_uid,
                buff_id: behavior.arg(0)?,
                amount: Some(amount),
                occurrences: 1,
                child_uid_reservations: 0,
            },
        ))));
    }
    Some(ops)
}
