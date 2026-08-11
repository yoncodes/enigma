use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Captures the committed player-action result consumed by Impromptu resolution.
pub struct ImpromptuResolution {
    pub team: i32,
    pub emitter_uid: i64,
    pub critical_action_count: i32,
}

pub fn run_player_actions_resolved(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    team: i32,
    emitter_uid: i64,
) -> Result<DrainResult, DrainError> {
    drain::run_event(
        managers,
        pool,
        catalog,
        determinism,
        context,
        BattleEvent::PlayerActionsResolved { team, emitter_uid },
    )
}

pub fn run_card_energy_clear(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> Result<DrainResult, DrainError> {
    drain::run_command_group(
        managers,
        pool,
        catalog,
        determinism,
        context,
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::ClearEnergy {
                origin: CARD_ENERGY_CLEAR_ORIGIN,
            },
        ))],
    )
}

pub fn run_impromptu_resolved(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    resolution: ImpromptuResolution,
) -> Result<DrainResult, DrainError> {
    drain::run_command_group(
        managers,
        pool,
        catalog,
        determinism,
        context,
        [
            RuleOp::SkillLifecycle(
                crate::engine::skill::action::SkillLifecycle::EmitterSkillEnded {
                    source_uid: resolution.emitter_uid,
                },
            ),
            RuleOp::Publish(BattleEvent::ImpromptuResolved {
                team: resolution.team,
                emitter_uid: resolution.emitter_uid,
                critical_action_count: resolution.critical_action_count,
            }),
        ],
    )
}

/// Resolves the configured Impromptu mechanic after player actions settle.
pub fn run_impromptu(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    team: i32,
    emitter_uid: i64,
) -> Result<DrainResult, DrainError> {
    let mut result = run_player_actions_resolved(
        managers,
        pool,
        catalog,
        determinism,
        context,
        team,
        emitter_uid,
    )?;
    let mut critical_action_count = 0;
    if let Some(plan) = impromptu::build_plan(managers, team, emitter_uid) {
        for attack_index in 1..=plan.attack_count {
            if pool
                .enemies(emitter_uid, false)
                .iter()
                .all(|target| managers.hp.current(target.uid) <= 0)
            {
                break;
            }
            let mut split_count =
                crate::engine::skill::buff_act::emitter_num_change::split_count_for(
                    &managers.buff,
                    &managers.hp,
                    emitter_uid,
                    attack_index,
                );
            let mut cast_context = context;
            cast_context.logic_target = catalog.logic_target(plan.skill_id);
            cast_context.active_skill_id = plan.skill_id;
            cast_context.active_skill_source_uid = emitter_uid;
            cast_context.active_skill_is_attack = true;
            cast_context.active_skill_effect_tag = catalog.effect_tag(plan.skill_id);
            cast_context.emitter_attack_index = attack_index;
            cast_context.emitter_attack_max = plan.attack_count;
            if crate::engine::skill::buff_act::emitter_rend_target::resolve(
                managers,
                pool,
                emitter_uid,
                attack_index,
                plan.attack_count,
            )
            .is_some()
            {
                split_count = 0;
            }
            cast_context.extra_damage_target_count = split_count;
            cast_context.extra_damage_target_final_damage_delta =
                crate::engine::skill::buff_act::emitter_num_change::split_final_damage_delta_for(
                    &managers.buff,
                    &managers.hp,
                    emitter_uid,
                    split_count,
                );
            cast_context.direct_skill_body = true;
            let mut invocation: crate::engine::skill::action::SkillInvocation =
                crate::engine::skill::action::SkillRequest {
                    source_uid: plan.source_uid,
                    skill_id: plan.skill_id,
                }
                .into();
            if attack_index == 1 {
                invocation.card_index = managers
                    .card
                    .played()
                    .iter()
                    .map(|played| played.card_index)
                    .max()
                    .unwrap_or_default()
                    .saturating_add(1);
            }
            invocation.rate_modifier =
                Some(crate::engine::skill::action::SkillRateModifier::fixed(
                    0,
                    plan.damage_rate_opcode,
                    plan.damage_rate,
                    true,
                ));
            let prelude = managers
                .emitter
                .begin_attack(
                    plan.source_uid,
                    attack_index,
                    plan.attack_count,
                    split_count,
                )
                .map(crate::engine::skill::action::SkillLifecycle::EmitterAttackStarted)
                .map(RuleOp::SkillLifecycle);
            let attack = drain::run_action(
                managers,
                pool,
                catalog,
                determinism,
                cast_context,
                prelude,
                invocation,
            )?;
            critical_action_count += i32::from(attack.outcomes.iter().any(|outcome| {
                matches!(
                    outcome,
                    RuleOutcome::SkillLifecycle(
                        crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action)
                    ) if action.crit_count > 0
                )
            }));
            append(&mut result, attack);
        }
        append(
            &mut result,
            run_impromptu_resolved(
                managers,
                pool,
                catalog,
                determinism,
                context,
                ImpromptuResolution {
                    team,
                    emitter_uid,
                    critical_action_count,
                },
            )?,
        );
    }
    let finalization = impromptu::finalize_action_queue_rule_ops(managers, team, emitter_uid);
    if !finalization.is_empty() {
        append(
            &mut result,
            drain::run_command_group(managers, pool, catalog, determinism, context, finalization)?,
        );
    }
    Ok(result)
}
