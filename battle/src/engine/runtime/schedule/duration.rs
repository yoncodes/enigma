use super::*;

pub(super) fn duration_advance_rule(take_stage: i32, snapshot: &[(i64, i64)]) -> Option<RuleOp> {
    (!snapshot.is_empty()).then(|| {
        RuleOp::Command(BattleCommand::BuffBatch(
            snapshot
                .iter()
                .map(|&(owner_uid, buff_uid)| duration_command(take_stage, owner_uid, buff_uid))
                .collect(),
        ))
    })
}

fn duration_command(take_stage: i32, owner_uid: i64, buff_uid: i64) -> BuffCommand {
    BuffCommand::AdvanceDuration(
        BuffDurationAdvance::new(take_stage, vec![owner_uid], Some(vec![buff_uid]))
            .expect("duration effect time is registered"),
    )
}

pub(super) fn run_round_start_duration(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    snapshot: &[(i64, i64)],
) -> Result<DrainResult, DrainError> {
    let Some(ops) = expiry_ops(managers, snapshot) else {
        return drain::run(
            managers,
            pool,
            catalog,
            determinism,
            context,
            duration_advance_rule(effect_time::ROUND_START_DURATION, snapshot),
        );
    };
    drain::run_command_group(managers, pool, catalog, determinism, context, ops)
}

fn expiry_ops(managers: &BattleManagers, snapshot: &[(i64, i64)]) -> Option<Vec<RuleOp>> {
    let take_stage = effect_time::ROUND_START_DURATION;
    let settlements = managers
        .buff
        .settling_expiries(take_stage, managers.fight_version());
    if !snapshot.iter().any(|timer| settlements.contains(timer)) {
        return None;
    }
    let mut ops = Vec::new();
    for timers in snapshot.chunk_by(|a, b| settlements.contains(a) == settlements.contains(b)) {
        if settlements.contains(&timers[0]) {
            ops.extend(timers.iter().map(|&(owner, uid)| {
                RuleOp::Command(BattleCommand::Buff(duration_command(
                    take_stage, owner, uid,
                )))
            }));
        } else {
            ops.extend(duration_advance_rule(take_stage, timers));
        }
    }
    Some(ops)
}

pub(super) fn run_round_start_durations(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    snapshot: &[(i64, i64)],
) -> Result<DrainResult, DrainError> {
    let Some(ops) = expiry_ops(managers, snapshot) else {
        let stages = effect_time::ROUND_START_DURATION_STAGES
            .into_iter()
            .filter_map(|stage| duration_advance_rule(stage, snapshot));
        return drain::run(managers, pool, catalog, determinism, context, stages);
    };
    let mut result = drain::run_command_group(managers, pool, catalog, determinism, context, ops)?;
    append(
        &mut result,
        drain::run(
            managers,
            pool,
            catalog,
            determinism,
            context,
            duration_advance_rule(effect_time::ROUND_START_AFTER_REACTION_DURATION, snapshot),
        )?,
    );
    Ok(result)
}

pub(super) fn duration_snapshot(managers: &BattleManagers, owner_uids: &[i64]) -> Vec<(i64, i64)> {
    owner_uids
        .iter()
        .flat_map(|owner_uid| {
            let mut buff_uids = managers
                .buff
                .active_for(*owner_uid)
                .filter_map(|buff| buff.uid)
                .collect::<Vec<_>>();
            buff_uids.sort_unstable();
            buff_uids
                .into_iter()
                .map(|buff_uid| (*owner_uid, buff_uid))
                .collect::<Vec<_>>()
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_owned_duration_advances(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    take_stage: i32,
    targets: Vec<crate::engine::manager::buff::BuffDurationTarget>,
) -> Result<DrainResult, DrainError> {
    drain::run_owned_command_group(
        managers,
        pool,
        catalog,
        determinism,
        context,
        owned_duration_ops(take_stage, targets),
    )
}

pub(super) fn owned_duration_ops(
    take_stage: i32,
    targets: Vec<crate::engine::manager::buff::BuffDurationTarget>,
) -> impl Iterator<Item = (FrameOwner, RuleOp)> {
    targets.into_iter().map(move |target| {
        let owner = FrameOwner::BuffDuration {
            owner_uid: target.owner_uid,
            buff_uid: target.buff_uid,
            take_stage,
        };
        let advance = duration_command(take_stage, target.owner_uid, target.buff_uid);
        (owner, RuleOp::Command(BattleCommand::Buff(advance)))
    })
}
