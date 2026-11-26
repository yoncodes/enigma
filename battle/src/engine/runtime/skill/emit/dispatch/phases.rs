use super::*;

pub(super) fn finish(
    invocation: &SkillInvocation,
    context: &EmissionContext<'_>,
    prepared: &Prepared<'_>,
    determinism: &mut RoundDeterminism,
    execution: &mut SkillExecution,
    emitted: SlotOutput,
) -> Result<SkillEmission, SkillOpError> {
    let EmissionContext {
        managers,
        pool,
        catalog,
        trigger,
    } = *context;
    let Prepared {
        effect,
        effect_skill_id,
        active_phase,
        source_team,
        has_row_damage,
        ..
    } = *prepared;
    let SlotOutput {
        ops: mut outputs,
        fired_rules,
    } = emitted;
    if active_phase == Some(SkillPhase::Immediate) {
        let prepares_target_buffs = !execution.modifiers.post_immediate_target_buffs.is_empty();
        let freeze_rates = execution
            .modifiers
            .rates
            .iter()
            .any(|modifier| modifier.fixed_value().is_none());
        let freeze = || SkillEmissionOp {
            op: RuleOp::FreezeActiveSkillRates,
            owner: behavior::registry::OutputOwner::Skill,
            consequence: ConsequencePolicy::Default,
            frame_owner: None,
        };
        if prepares_target_buffs && freeze_rates {
            outputs.push(freeze());
        }
        outputs.extend(
            execution
                .modifiers
                .post_immediate_target_buffs
                .iter()
                .flat_map(|modifier| {
                    execution
                        .configured_targets
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .map(|target_uid| SkillEmissionOp {
                            op: RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
                                BuffGrant {
                                    origin: modifier.origin,
                                    source_uid: invocation.plan.source_uid,
                                    target_uid: *target_uid,
                                    buff_id: modifier.buff_id,
                                    amount: Some(modifier.amount),
                                    occurrences: 1,
                                    child_uid_reservations: 0,
                                },
                            ))),
                            owner: behavior::registry::OutputOwner::Skill,
                            consequence: ConsequencePolicy::Default,
                            frame_owner: None,
                        })
                }),
        );
        let prepared = (catalog.has_preparation_observers()
            || managers
                .buff
                .observes(crate::engine::event::kind::EventKind::SkillEffectsPrepared))
        .then(|| effects_prepared_op(invocation, managers, catalog, pool, execution));
        let phase_completed = phase_completed_op(
            invocation,
            managers,
            catalog,
            pool,
            execution,
            SkillPhase::Immediate,
        );
        outputs.extend(super::super::payment::settle(
            invocation,
            execution,
            phase_completed,
            prepared,
        ));
        if !prepares_target_buffs && freeze_rates {
            outputs.push(freeze());
        }
    }
    if active_phase == Some(SkillPhase::Immediate) && has_row_damage {
        for activation in plan::LinkedDamagePlan::activations(invocation, managers, execution) {
            let activated = execution.has_activated_additional_damage(&activation.additional);
            if !activated {
                outputs.extend(emit_additional_damage_activation(
                    invocation, execution, activation,
                ));
            }
        }
    }
    let mut has_after_damage = false;
    let mut has_after_hit = false;
    for slot in &effect.slots {
        let Some(definition) = behavior::registry::find(&slot.behavior) else {
            continue;
        };
        let routed_phases =
            slot.active_phases()
                .map_err(|route| SkillOpError::UncompiledRoute {
                    skill_id: invocation.plan.skill_id,
                    route,
                })?;
        let runs_in = |phase| {
            if routed_phases.is_empty() {
                definition.phase == phase
            } else {
                routed_phases.contains(&phase)
            }
        };
        has_after_damage |= runs_in(SkillPhase::AfterDamage);
        has_after_hit |= runs_in(SkillPhase::AfterHit);
    }
    has_after_damage |= execution.has_after_damage_ops();
    if matches!(
        active_phase,
        Some(SkillPhase::Immediate | SkillPhase::AdditionalDamage)
    ) {
        let subscribers = crate::engine::skill::subscriber::for_compiled_owner_events(
            pool,
            managers,
            catalog,
            [crate::engine::event::kind::EventKind::SkillAction],
            &[invocation.plan.source_uid],
        )
        .map_err(SkillOpError::from)?;
        has_after_damage |= subscribers
            .skills
            .into_iter()
            .any(|subscriber| subscriber.key.phase == Some(SkillPhase::AfterDamage))
            || subscribers
                .buff_acts
                .into_iter()
                .any(|subscriber| subscriber.key.phase == Some(SkillPhase::AfterDamage));
    }
    if active_phase == Some(crate::engine::skill::action::SkillPhase::Damage) && has_row_damage {
        let damage = plan::damage_ops(
            invocation,
            managers,
            pool,
            catalog,
            effect_skill_id,
            determinism,
            execution,
        )?;
        let damage_buff_act_frame_owner = damage.buff_act_frame_owner;
        execution.pending_additional_damage = damage.additional_damage;
        execution.set_after_damage_ops(damage.after_damage, damage_buff_act_frame_owner.clone());
        if damage.main_target.is_some() {
            execution.primary_target_uid = damage.main_target;
        }
        if let Some(key) = execution.take_team_injury_count_consumption() {
            outputs.push(SkillEmissionOp {
                op: RuleOp::Command(BattleCommand::Injury(
                    crate::engine::manager::injury::InjuryCommand::Reset {
                        origin: crate::engine::skill::rule::CommandOrigin {
                            domain: crate::engine::skill::rule::RuleDomain::Condition,
                            key,
                        },
                        team_type: source_team,
                    },
                )),
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            });
        }
        for feature in damage.avoided {
            let act_id = feature
                .act_id()
                .ok_or_else(|| SkillOpError::UnregisteredBuffAct {
                    opcode: 0,
                    type_name: feature.act_type.clone(),
                })?;
            let ops = crate::engine::skill::buff_act::dodge_spec_skill::trigger_rule_ops(&feature)
                .ok_or_else(|| SkillOpError::UnregisteredBuffAct {
                    opcode: act_id,
                    type_name: feature.act_type.clone(),
                })?;
            execution.record_triggered_buff_feature(
                crate::engine::event::payload::BuffFeatureTriggeredEvent {
                    owner_uid: feature.owner_uid,
                    source_uid: feature.source_uid,
                    target_uid: feature.owner_uid,
                    buff_uid: feature.buff_uid,
                    buff_id: feature.buff_id,
                    act_id,
                },
            );
            outputs.extend(ops.into_iter().map(|op| SkillEmissionOp {
                op,
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            }));
        }
        outputs.extend(damage.before_damage.into_iter().map(|op| SkillEmissionOp {
            op,
            owner: behavior::registry::OutputOwner::Skill,
            consequence: ConsequencePolicy::Default,
            frame_owner: damage_buff_act_frame_owner.clone(),
        }));
        if execution.modifiers.force_critical {
            outputs.extend(authored_critical_markers(&damage.damage));
        }
        if !damage.damage.is_empty() {
            outputs.push(SkillEmissionOp {
                op: RuleOp::Command(BattleCommand::HpBatch(damage.damage)),
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            });
        }
    }
    if active_phase == Some(crate::engine::skill::action::SkillPhase::AdditionalDamage) {
        let additional_damage_commands = execution.take_additional_damage();
        if !additional_damage_commands.is_empty() {
            outputs.push(SkillEmissionOp {
                op: RuleOp::Command(BattleCommand::LinkedDamage(additional_damage_commands)),
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            });
        }
        for additional in &execution.modifiers.additional_damage {
            let Some((feature, _)) = crate::engine::skill::buff_act::additional_damage::configured(
                managers.catalog(),
                additional.buff_id,
                invocation.plan.source_uid,
                invocation.plan.source_uid,
            ) else {
                continue;
            };
            let Some(origin) = crate::engine::skill::buff_act::feature_command_origin(&feature)
            else {
                continue;
            };
            outputs.push(SkillEmissionOp {
                op: RuleOp::Command(BattleCommand::Buff(BuffCommand::Deactivate(BuffRemove {
                    origin,
                    target_uid: invocation.plan.source_uid,
                    selector: BuffRemoveSelector::ExactId(additional.buff_id),
                }))),
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: Some(crate::engine::runtime::record::FrameOwner::BuffAct {
                    owner_uid: feature.owner_uid,
                    source_uid: feature.source_uid,
                    buff_uid: feature.buff_uid,
                    buff_id: feature.buff_id,
                    key: origin.key,
                }),
            });
        }
    }
    if active_phase == Some(crate::engine::skill::action::SkillPhase::AfterDamage) {
        let (after_damage_ops, frame_owner) = execution.take_after_damage_ops();
        outputs.extend(after_damage_ops.into_iter().map(|op| SkillEmissionOp {
            op,
            owner: behavior::registry::OutputOwner::Skill,
            consequence: ConsequencePolicy::Default,
            frame_owner: frame_owner.clone(),
        }));
    }
    let next_phase = match active_phase {
        Some(crate::engine::skill::action::SkillPhase::SourceOpening) => {
            Some(crate::engine::skill::action::SkillPhase::Immediate)
        }
        Some(crate::engine::skill::action::SkillPhase::Immediate) if has_row_damage => {
            Some(crate::engine::skill::action::SkillPhase::Damage)
        }
        Some(crate::engine::skill::action::SkillPhase::Immediate) if has_after_damage => {
            Some(crate::engine::skill::action::SkillPhase::AfterDamage)
        }
        Some(crate::engine::skill::action::SkillPhase::Immediate) if has_after_hit => {
            Some(crate::engine::skill::action::SkillPhase::HitPassives)
        }
        Some(crate::engine::skill::action::SkillPhase::Immediate) => {
            Some(crate::engine::skill::action::SkillPhase::HitPassives)
        }
        Some(crate::engine::skill::action::SkillPhase::Damage) => {
            Some(crate::engine::skill::action::SkillPhase::AdditionalDamage)
        }
        Some(crate::engine::skill::action::SkillPhase::AdditionalDamage) if has_after_damage => {
            Some(crate::engine::skill::action::SkillPhase::AfterDamage)
        }
        Some(crate::engine::skill::action::SkillPhase::AdditionalDamage) => {
            Some(crate::engine::skill::action::SkillPhase::HitPassives)
        }
        Some(crate::engine::skill::action::SkillPhase::AfterDamage)
            if has_after_hit || has_row_damage =>
        {
            Some(crate::engine::skill::action::SkillPhase::HitPassives)
        }
        Some(crate::engine::skill::action::SkillPhase::AfterDamage) => {
            Some(crate::engine::skill::action::SkillPhase::HitPassives)
        }
        Some(crate::engine::skill::action::SkillPhase::HitPassives) => {
            Some(crate::engine::skill::action::SkillPhase::AfterHit)
        }
        _ => None,
    };
    let continuation = next_phase.map(|phase| {
        let mut continuation = invocation.clone();
        continuation.phase = Some(phase);
        continuation.additional_moxie = execution.context.additional_moxie;
        continuation
    });
    let publishes_lifecycle =
        invocation.condition_key.is_none() && !matches!(trigger, SkillOpTrigger::Setup { .. });

    if publishes_lifecycle && active_phase == Some(SkillPhase::SourceOpening) {
        outputs.push(phase_completed_op(
            invocation,
            managers,
            catalog,
            pool,
            execution,
            SkillPhase::SourceOpening,
        ));
    } else if publishes_lifecycle
        && active_phase == Some(SkillPhase::AdditionalDamage)
        && has_row_damage
    {
        outputs.push(phase_completed_op(
            invocation,
            managers,
            catalog,
            pool,
            execution,
            SkillPhase::AdditionalDamage,
        ));
    } else if publishes_lifecycle && active_phase == Some(SkillPhase::HitPassives) {
        outputs.push(phase_completed_op(
            invocation,
            managers,
            catalog,
            pool,
            execution,
            SkillPhase::HitPassives,
        ));
    } else if publishes_lifecycle && active_phase == Some(SkillPhase::AfterDamage) {
        outputs.push(phase_completed_op(
            invocation,
            managers,
            catalog,
            pool,
            execution,
            SkillPhase::AfterDamage,
        ));
        if continuation.is_none() {
            outputs.push(phase_completed_op(
                invocation,
                managers,
                catalog,
                pool,
                execution,
                SkillPhase::AfterHit,
            ));
        }
    } else if publishes_lifecycle && active_phase == Some(SkillPhase::AfterHit) {
        outputs.push(phase_completed_op(
            invocation,
            managers,
            catalog,
            pool,
            execution,
            SkillPhase::AfterHit,
        ));
    } else if publishes_lifecycle && active_phase.is_some() && continuation.is_none() {
        outputs.push(phase_completed_op(
            invocation,
            managers,
            catalog,
            pool,
            execution,
            SkillPhase::AfterDamage,
        ));
        outputs.push(phase_completed_op(
            invocation,
            managers,
            catalog,
            pool,
            execution,
            SkillPhase::AfterHit,
        ));
    }
    if active_phase == Some(SkillPhase::HitPassives) {
        outputs.extend(
            execution
                .take_triggered_buff_features()
                .into_iter()
                .map(|event| SkillEmissionOp {
                    op: RuleOp::Publish(
                        crate::engine::event::payload::BattleEvent::BuffFeatureTriggered(event),
                    ),
                    owner: behavior::registry::OutputOwner::Skill,
                    consequence: ConsequencePolicy::Default,
                    frame_owner: None,
                }),
        );
    }
    if active_phase == Some(crate::engine::skill::action::SkillPhase::AfterHit) {
        for (origin, buff_id) in std::mem::take(&mut execution.temporary_damage_buffs) {
            outputs.push(SkillEmissionOp {
                op: RuleOp::Command(BattleCommand::Buff(BuffCommand::Deactivate(BuffRemove {
                    origin,
                    target_uid: invocation.plan.source_uid,
                    selector: BuffRemoveSelector::ExactId(buff_id),
                }))),
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            });
        }
    }
    if continuation.is_none() && invocation.mode.publishes_completion() {
        outputs.push(SkillEmissionOp {
            op: RuleOp::SkillLifecycle(
                crate::engine::skill::action::SkillLifecycle::ActionCompleted(
                    crate::engine::skill::action::ActionEvent {
                        source_uid: invocation.plan.source_uid,
                        skill_id: invocation.plan.skill_id,
                        target_uid: match invocation.target {
                            SkillTarget::Explicit(selected)
                                if !execution.configured_target_replaces(
                                    invocation.target,
                                    invocation.plan.source_uid,
                                    pool,
                                ) =>
                            {
                                selected
                            }
                            _ => execution.primary_target_uid.unwrap_or_default(),
                        },
                        target_uids: execution.affected_targets.clone(),
                        skill_slot: pool.skill_slot(
                            managers,
                            invocation.plan.source_uid,
                            invocation.plan.skill_id,
                        ),
                        is_attack: catalog.is_attack(effect_skill_id),
                        rank: managers.catalog().skill_rank(invocation.plan.skill_id),
                        skill_type: execution.context.damage_target_count_kind,
                        effect_tag: catalog.effect_tag(effect_skill_id),
                        additional_moxie: execution.context.additional_moxie,
                        moxie_spent: execution
                            .moxie_spent
                            .saturating_add(execution.context.additional_moxie.max(0))
                            .max(invocation.moxie_spent.max(0)),
                        trigger: invocation.trigger,
                        extra_skill_kind: execution.context.extra_skill_kind,
                        mode: invocation.mode,
                        is_interval: catalog.is_interval(invocation.plan.skill_id),
                        assassinate: execution.context.active_skill_assassinate,
                        damage_amount: execution.context.action_damage_amount,
                        kill_count: execution.context.action_kill_count,
                        crit_count: execution.context.action_crit_count,
                        teammate_injury_count: execution.injured_allies.len() as i32,
                        teammate_injury_count_not_reset: execution.injured_allies.len() as i32,
                        team_injury_count_round: execution.team_injury_count_round,
                        card_enchants: invocation.card_enchants.clone(),
                        card_index: invocation.card_index,
                    },
                ),
            ),
            owner: behavior::registry::OutputOwner::Skill,
            consequence: ConsequencePolicy::Default,
            frame_owner: None,
        });
    }
    Ok(SkillEmission {
        ops: outputs,
        fired_rules,
        continuation,
        target_uid: execution.primary_target_uid,
    })
}

fn authored_critical_markers(
    commands: &[crate::engine::manager::hp::HpCommand],
) -> impl Iterator<Item = SkillEmissionOp> + '_ {
    use crate::engine::manager::hp::{DamageEffectKind, HpCommand};

    commands.iter().filter_map(|command| match command {
        HpCommand::Damage(hit) if hit.effect_kind == DamageEffectKind::Critical => {
            Some(SkillEmissionOp {
                op: RuleOp::EffectMarker {
                    target_uid: hit.source_uid,
                    effect_type: sonettobuf::effect_type_enum::EffectType::Mustcrit as i32,
                    effect_num: 0,
                    effect_num1: None,
                    config_effect: 0,
                    reserve_id: Some(hit.target_uid),
                    reserve_str: None,
                },
                owner: behavior::registry::OutputOwner::Skill,
                consequence: ConsequencePolicy::Default,
                frame_owner: None,
            })
        }
        _ => None,
    })
}
