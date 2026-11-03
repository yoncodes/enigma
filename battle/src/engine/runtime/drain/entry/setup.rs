use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SetupFrameContainer {
    Standalone,
    RoundPhase,
    OpeningRoundPhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SetupSkillFilter {
    All,
    ParentOnly,
    ExcludeParent,
}

impl SetupSkillFilter {
    fn includes(self, behavior: &crate::engine::skill::effect::ParsedBehavior) -> bool {
        let setup_parent =
            crate::engine::skill::behavior::registry::find(behavior).is_some_and(|definition| {
                definition.output_owner
                    == crate::engine::skill::behavior::registry::OutputOwner::SetupParent
            });
        match self {
            Self::All => true,
            Self::ParentOnly => setup_parent,
            Self::ExcludeParent => !setup_parent,
        }
    }
}

impl SetupFrameContainer {
    pub(super) fn owns_entity_scope(
        self,
        registered_scope: Option<crate::engine::skill::condition::registry::SetupFrameScope>,
        owner_uid: i64,
    ) -> bool {
        self == Self::RoundPhase
            || crate::engine::fight::rules::is_side_uid(owner_uid)
            || registered_scope
                == Some(crate::engine::skill::condition::registry::SetupFrameScope::Side)
    }

    pub(super) fn roots_entity_scope(
        self,
        registered_scope: Option<crate::engine::skill::condition::registry::SetupFrameScope>,
        owner_uid: i64,
    ) -> bool {
        self == Self::OpeningRoundPhase && !self.owns_entity_scope(registered_scope, owner_uid)
    }
}

pub fn run_setup_stage(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    stage: SetupStage,
    priority: i32,
) -> Result<DrainResult, DrainError> {
    run_skill_then_buff_act_setup_stage_filtered(
        managers,
        pool,
        catalog,
        determinism,
        context,
        stage,
        priority,
        None,
        &[],
        false,
        SetupSkillFilter::All,
        true,
        SetupFrameContainer::Standalone,
        DefenderCardCleanupOwner::DeathAction,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_setup_stage_for_owners(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    stage: SetupStage,
    priority: i32,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    run_skill_then_buff_act_setup_stage_filtered(
        managers,
        pool,
        catalog,
        determinism,
        context,
        stage,
        priority,
        Some(owner_uids),
        &[],
        false,
        SetupSkillFilter::All,
        true,
        SetupFrameContainer::Standalone,
        DefenderCardCleanupOwner::DeathAction,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run_opening_setup_stage_for_owners(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    stage: SetupStage,
    priority: i32,
    player_owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    run_skill_then_buff_act_setup_stage_filtered(
        managers,
        pool,
        catalog,
        determinism,
        context,
        stage,
        priority,
        Some(player_owner_uids),
        &[],
        true,
        SetupSkillFilter::All,
        true,
        SetupFrameContainer::Standalone,
        DefenderCardCleanupOwner::DeathAction,
    )
}

#[derive(Default)]
struct SetupStageState {
    result: DrainResult,
    side_paths: Vec<(SetupSide, FramePath)>,
}

impl SetupStageState {
    fn append(&mut self, next: DrainResult) {
        self.result.outcomes.extend(next.outcomes);
        self.result.events.extend(next.events);
        self.result.frames = next.frames;
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_skill_then_buff_act_setup_stage_filtered(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    stage: SetupStage,
    priority: i32,
    owner_uids: Option<&[i64]>,
    excluded_owner_uids: &[i64],
    include_both_sides_opening: bool,
    skill_filter: SetupSkillFilter,
    include_buff_acts: bool,
    frame_container: SetupFrameContainer,
    cleanup_owner: DefenderCardCleanupOwner,
) -> Result<DrainResult, DrainError> {
    let mut state = SetupStageState::default();
    append_skill_setup_stage(
        managers,
        pool,
        catalog,
        determinism,
        context,
        stage,
        priority,
        owner_uids,
        excluded_owner_uids,
        include_both_sides_opening,
        skill_filter,
        frame_container,
        cleanup_owner,
        &mut state,
    )?;
    if include_buff_acts {
        append_buff_act_setup_stage(
            managers,
            pool,
            catalog,
            determinism,
            context,
            stage,
            priority,
            owner_uids,
            excluded_owner_uids,
            cleanup_owner,
            &mut state,
        )?;
    }
    Ok(state.result)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_skill_setup_stage_filtered(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    stage: SetupStage,
    priority: i32,
    owner_uids: Option<&[i64]>,
    excluded_owner_uids: &[i64],
    include_both_sides_opening: bool,
    frame_container: SetupFrameContainer,
    cleanup_owner: DefenderCardCleanupOwner,
) -> Result<DrainResult, DrainError> {
    let mut state = SetupStageState::default();
    append_skill_setup_stage(
        managers,
        pool,
        catalog,
        determinism,
        context,
        stage,
        priority,
        owner_uids,
        excluded_owner_uids,
        include_both_sides_opening,
        SetupSkillFilter::All,
        frame_container,
        cleanup_owner,
        &mut state,
    )?;
    Ok(state.result)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn run_buff_act_setup_stage_filtered(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    stage: SetupStage,
    priority: i32,
    owner_uids: Option<&[i64]>,
    excluded_owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    let mut state = SetupStageState::default();
    append_buff_act_setup_stage(
        managers,
        pool,
        catalog,
        determinism,
        context,
        stage,
        priority,
        owner_uids,
        excluded_owner_uids,
        DefenderCardCleanupOwner::DeathAction,
        &mut state,
    )?;
    Ok(state.result)
}

#[allow(clippy::too_many_arguments)]
fn append_skill_setup_stage(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    stage: SetupStage,
    priority: i32,
    owner_uids: Option<&[i64]>,
    excluded_owner_uids: &[i64],
    include_both_sides_opening: bool,
    skill_filter: SetupSkillFilter,
    frame_container: SetupFrameContainer,
    cleanup_owner: DefenderCardCleanupOwner,
    state: &mut SetupStageState,
) -> Result<(), DrainError> {
    let context = context_for_setup_stage(context, stage);
    let trigger = FrameTrigger::Setup { stage, priority };
    let current_pool = pool.runtime_view(managers);
    let subscribers =
        dispatcher::dispatch_compiled_setup(&current_pool, managers, catalog, stage, priority)?;
    let subscribers = subscribers
            .into_iter()
            .filter(|(subscriber, _)| {
                let included = catalog
                    .get(subscriber.skill_id)
                    .and_then(|effect| effect.slots.get(subscriber.slot_index))
                    .is_some_and(|slot| skill_filter.includes(&slot.behavior));
                !excluded_owner_uids.contains(&subscriber.owner_uid)
                    && included
                    && (owner_uids.is_none_or(|uids| uids.contains(&subscriber.owner_uid))
                        || (include_both_sides_opening
                            && crate::engine::skill::condition::registry::opening_owner_eligibility(
                                subscriber.key.opcode,
                                subscriber.key.type_name,
                            )
                            .is_some_and(|eligibility| {
                                eligibility
                                    == crate::engine::skill::condition::registry::OpeningOwnerEligibility::BothSides
                            })))
            });
    let subscribers = subscribers.collect::<Vec<_>>();
    let mut frames = std::mem::take(&mut state.result.frames);
    let side_paths = &mut state.side_paths;
    let mut owner_paths = Vec::<(SetupSide, i64, FramePath)>::new();
    let mut skill_frame_groups = Vec::<(i64, i32, Rc<RefCell<Option<FramePath>>>)>::new();
    let mut queue = VecDeque::new();
    for (subscriber, op) in subscribers {
        let side = if current_pool.source_is_attacker(subscriber.owner_uid) {
            SetupSide::Attacker
        } else {
            SetupSide::Defender
        };
        let registered_scope = crate::engine::skill::condition::registry::find_key(
            subscriber.key.opcode,
            subscriber.key.type_name,
        )
        .map(|definition| definition.setup_frame_scope);
        let owner_path =
            if frame_container.roots_entity_scope(registered_scope, subscriber.owner_uid) {
                match owner_paths.iter().find(|(known_side, uid, _)| {
                    *known_side == side && *uid == subscriber.owner_uid
                }) {
                    Some((_, _, path)) => path.clone(),
                    None => {
                        let path = push_root(
                            &mut frames,
                            FrameOwner::SetupEntity {
                                owner_uid: subscriber.owner_uid,
                            },
                            trigger.clone(),
                        );
                        owner_paths.push((side, subscriber.owner_uid, path.clone()));
                        path
                    }
                }
            } else {
                let side_path = match side_paths.iter().find(|(known, _)| *known == side) {
                    Some((_, path)) => path.clone(),
                    None => {
                        let path =
                            push_root(&mut frames, FrameOwner::SetupSide(side), trigger.clone());
                        side_paths.push((side, path.clone()));
                        path
                    }
                };
                if frame_container.owns_entity_scope(registered_scope, subscriber.owner_uid) {
                    side_path
                } else {
                    match owner_paths.iter().find(|(known_side, uid, _)| {
                        *known_side == side && *uid == subscriber.owner_uid
                    }) {
                        Some((_, _, path)) => path.clone(),
                        None => {
                            let path = push_child(
                                &mut frames,
                                &side_path,
                                FrameOwner::SetupEntity {
                                    owner_uid: subscriber.owner_uid,
                                },
                                trigger.clone(),
                            );
                            owner_paths.push((side, subscriber.owner_uid, path.clone()));
                            path
                        }
                    }
                }
            };
        let frame_group = match skill_frame_groups.iter().find(|(owner_uid, skill_id, _)| {
            *owner_uid == subscriber.owner_uid && *skill_id == subscriber.skill_id
        }) {
            Some((_, _, group)) => Rc::clone(group),
            None => {
                let group = Rc::new(RefCell::new(None));
                skill_frame_groups.push((
                    subscriber.owner_uid,
                    subscriber.skill_id,
                    Rc::clone(&group),
                ));
                group
            }
        };
        queue.push_back(QueuedOp {
            op,
            trigger: SkillOpTrigger::Setup { stage, priority },
            skill_execution: None,
            frame_path: None,
            parent_path: Some(owner_path),
            frame_group: Some(frame_group),
            independent_parent_group: None,
            frame_owner: Some(FrameOwner::Skill {
                source_uid: subscriber.owner_uid,
                skill_id: subscriber.skill_id,
                card_index: 0,
                target_uid: Some(subscriber.owner_uid),
            }),
            subscriber: None,
            caster_frame: None,
        });
    }
    let stage_result = drain_queue_with_frames_and_cleanup_owner(
        managers,
        pool,
        catalog,
        determinism,
        context,
        &mut queue,
        frames,
        cleanup_owner,
    )?;
    state.append(stage_result);
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_buff_act_setup_stage(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    stage: SetupStage,
    priority: i32,
    owner_uids: Option<&[i64]>,
    excluded_owner_uids: &[i64],
    cleanup_owner: DefenderCardCleanupOwner,
    state: &mut SetupStageState,
) -> Result<(), DrainError> {
    let context = context_for_setup_stage(context, stage);
    let trigger = FrameTrigger::Setup { stage, priority };
    let mut frames = std::mem::take(&mut state.result.frames);
    let side_paths = &mut state.side_paths;
    let mut queue = VecDeque::new();
    let setup_subscribers = dispatcher::dispatch_buff_act_setup(managers, catalog, stage, priority);
    let setup_subscribers = setup_subscribers
        .into_iter()
        .filter(|(subscriber, _)| {
            !excluded_owner_uids.contains(&subscriber.feature.owner_uid)
                && owner_uids.is_none_or(|uids| uids.contains(&subscriber.feature.owner_uid))
        })
        .map(|(subscriber, ops)| {
            let definition = required_buff_act_definition(
                Some(subscriber.key.opcode),
                subscriber.key.type_name,
            )?;
            Ok((subscriber, ops, definition))
        })
        .collect::<Result<Vec<_>, DrainError>>()?;
    let mut setup_subscribers = setup_subscribers;
    setup_subscribers.sort_by_key(|(_, _, definition)| {
        let (scope, order) = definition.setup_frame(stage, priority);
        (
            matches!(
                scope,
                crate::engine::skill::buff_act::registry::SetupFrameScope::IndependentStep
            ),
            order,
        )
    });
    for (subscriber, ops, definition) in setup_subscribers {
        let ops = ops.ok_or(DrainError::MissingBuffActOp(subscriber.key.opcode))?;
        let feature = subscriber.feature;
        let buff_act_path = if matches!(
            definition.setup_frame(stage, priority).0,
            crate::engine::skill::buff_act::registry::SetupFrameScope::IndependentStep
        ) {
            push_root(
                &mut frames,
                FrameOwner::SetupBuffAct {
                    owner_uid: feature.owner_uid,
                    buff_id: feature.buff_id,
                    key: subscriber.key,
                },
                trigger.clone(),
            )
        } else {
            let side = if pool.source_is_attacker(feature.owner_uid) {
                SetupSide::Attacker
            } else {
                SetupSide::Defender
            };
            let side_path = match side_paths.iter().find(|(known, _)| *known == side) {
                Some((_, path)) => path.clone(),
                None => {
                    let path = push_root(&mut frames, FrameOwner::SetupSide(side), trigger.clone());
                    side_paths.push((side, path.clone()));
                    path
                }
            };
            match definition.setup_frame(stage, priority).0 {
                crate::engine::skill::buff_act::registry::SetupFrameScope::RootMechanicFrame => {
                    push_root(&mut frames, FrameOwner::SetupMechanic, trigger.clone())
                }
                crate::engine::skill::buff_act::registry::SetupFrameScope::MechanicFrame => {
                    push_child(
                        &mut frames,
                        &side_path,
                        FrameOwner::SetupMechanic,
                        trigger.clone(),
                    )
                }
                crate::engine::skill::buff_act::registry::SetupFrameScope::SubscriberFrame => {
                    let owner_path = push_child(
                        &mut frames,
                        &side_path,
                        FrameOwner::SetupEntity {
                            owner_uid: feature.owner_uid,
                        },
                        trigger.clone(),
                    );
                    push_child(
                        &mut frames,
                        &owner_path,
                        FrameOwner::BuffAct {
                            owner_uid: feature.owner_uid,
                            source_uid: feature.source_uid,
                            buff_uid: feature.buff_uid,
                            buff_id: feature.buff_id,
                            key: subscriber.key,
                        },
                        trigger.clone(),
                    )
                }
                crate::engine::skill::buff_act::registry::SetupFrameScope::IndependentStep => {
                    unreachable!("independent setup routes use a root frame")
                }
            }
        };
        for op in ops {
            let (frame_path, parent_path) = match op {
                RuleOp::Skill(_) => (None, Some(buff_act_path.clone())),
                _ => (Some(buff_act_path.clone()), None),
            };
            queue.push_back(QueuedOp {
                op,
                trigger: SkillOpTrigger::Setup { stage, priority },
                skill_execution: None,
                frame_path,
                parent_path,
                frame_group: None,
                independent_parent_group: None,
                frame_owner: None,
                subscriber: None,
                caster_frame: None,
            });
        }
    }
    let buff_act_result = drain_queue_with_frames_and_cleanup_owner(
        managers,
        pool,
        catalog,
        determinism,
        context,
        &mut queue,
        frames,
        cleanup_owner,
    )?;
    state.append(buff_act_result);
    Ok(())
}

pub(super) fn context_for_setup_stage(
    mut context: TargetContext,
    stage: SetupStage,
) -> TargetContext {
    if stage == SetupStage::RoundTransitionStart {
        context.current_round = context.current_round.saturating_sub(1).max(0);
    }
    context
}
