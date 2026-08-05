//! Owns when declared setup, actions, transitions, and settlement run.
//! Scheduling orders semantic work; it does not define rule meaning or mutate protobuf output.

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffDurationAdvance},
        card::{
            CARD_ENERGY_CLEAR_ORIGIN, CARD_PLAY_ORIGIN, CardCommand, CardInvalidatePlayed,
            CardRefillOne, CardRefreshAiQueue, CardSetup,
        },
        eureka::{EurekaChange, EurekaCommand, PowerType},
        ex_point::{ExPointChange, ExPointCommand},
    },
    mechanic::impromptu,
    round::command::RoundCommand,
    runtime::{
        change::BattleChange,
        determinism::{AiSkillChoice, RoundDeterminism},
        drain::{self, DrainError, DrainResult},
        executor::RuleOutcome,
        record::{
            CardInvalidReason, FrameItem, FrameOwner, FrameTrigger, RoundCue, RoundPhase,
            SemanticFrame, push_attributed_cue, push_cue, push_cues,
        },
    },
    skill::{
        buff_act::{self, damage_over_time, effect_time},
        effect::SkillEffectCatalog,
        rule::{
            CommandOrigin, DefinitionKey, RuleDomain, SetupStage,
            output::{BattleCommand, RuleOp},
        },
        target::{TargetContext, TargetPool},
    },
};

pub const START: &[(SetupStage, i32)] = &[
    (SetupStage::BattleStart, 0),
    (SetupStage::EnterFight, 0),
    (SetupStage::Unconditional, 0),
    (SetupStage::RoundStart, -1),
    (SetupStage::RoundStartCondition, 100),
    (SetupStage::RoundStartCondition, 101),
    (SetupStage::RoundStartCondition, 102),
    (SetupStage::RoundStart, 1),
    (SetupStage::RoundStart, 2),
    (SetupStage::BuffGate, 0),
    (SetupStage::BuffSync, 0),
    (SetupStage::RoundStartLate, 0),
    (SetupStage::RoundStart, 3),
    (SetupStage::CardSetup, 0),
    (SetupStage::AfterRoundStart, 0),
    (SetupStage::RoundStart, 4),
];

const ROUND_START_EVENT_SETUP: &[(SetupStage, i32)] = &[
    (SetupStage::RoundStart, 1),
    (SetupStage::RoundTransitionStart, 1),
];

const ROUND_START_BEFORE_DURATION_SETUP: &[(SetupStage, i32)] = &[
    (SetupStage::RoundStart, -1),
    (SetupStage::RoundStartCondition, 100),
    (SetupStage::RoundStartCondition, 101),
    (SetupStage::RoundStartCondition, 102),
];

fn opening_setup(version: i32) -> Vec<(SetupStage, i32)> {
    let mut setup = START.to_vec();
    if crate::engine::fight::versions::round_start_setup_layout(version)
        == Some(crate::engine::fight::versions::RoundStartSetupLayout::Version7)
    {
        let round_start = setup
            .iter()
            .position(|step| *step == (SetupStage::RoundStart, -1))
            .expect("opening setup has an early round-start stage");
        let round_start = setup.remove(round_start);
        let conditions_end = setup
            .iter()
            .position(|step| *step == (SetupStage::RoundStartCondition, 102))
            .expect("opening setup has the last round-start condition");
        setup.insert(conditions_end + 1, round_start);
    }
    setup
}

const ROUND_START_INDEPENDENT_SETUP: &[(SetupStage, i32)] = &[(SetupStage::EnterBattleStatic, 0)];

const ROUND_START_SETTLEMENT_SETUP: &[(SetupStage, i32)] = &[(SetupStage::RoundStart, 3)];

const ROUND_START_THRESHOLD_SETUP: &[(SetupStage, i32)] = &[(SetupStage::RoundStart, 4)];

const HAND_LIMIT_CLEANUP_ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Lifecycle,
    key: DefinitionKey::new(0, "RoundHandLimitCleanup"),
};

const ROUND_START_VERSION6_SYNC_SETUP: &[(SetupStage, i32)] =
    &[(SetupStage::RoundStart, 4), (SetupStage::BuffSync, 0)];

const ROUND_START_VERSION7_SYNC_SETUP: &[(SetupStage, i32)] = &[(SetupStage::BuffSync, 0)];

mod player;
mod refill;
mod resolution;
mod settlement;
mod start;

pub(crate) use player::card_skill_is_blocked;
pub use player::*;
use player::{
    ActiveActionRequest, card_play_resource_delta, run_active_action, run_card_composition_rewards,
};
use refill::run_opening_hand_refill;
pub use refill::*;
pub use resolution::*;
pub use settlement::*;
pub use start::*;
#[cfg(test)]
use start::{RoundStartSettlementPlan, raspberry_losses, run_round_start_owner_settlement};

pub fn run_wave_start_triggers(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    wave: i32,
) -> Result<DrainResult, DrainError> {
    let actions = crate::engine::fight::trigger::wave_start_actions(
        config::configs::get(),
        context.battle_id,
        wave,
    )?;
    if actions.is_empty() {
        return Ok(DrainResult::default());
    }

    let mut emitted = drain::run(
        managers,
        pool,
        catalog,
        determinism,
        context,
        actions.into_iter().map(|action| action.rule_op()),
    )?;
    let mut result = DrainResult::default();
    let root = crate::engine::runtime::record::push_root(
        &mut result.frames,
        FrameOwner::StageWave { wave },
        FrameTrigger::Active,
    );
    let root = result
        .frames
        .get_mut(root[0])
        .expect("wave-start trigger has a stage-wave owner");
    root.items.extend(
        emitted
            .frames
            .drain(..)
            .map(|frame| FrameItem::Child(Box::new(frame))),
    );
    result.outcomes = emitted.outcomes;
    result.events = emitted.events;
    Ok(result)
}

pub fn run_round_end_settlement(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    let mut result = begin_round_phase(RoundPhase::EntitySettlement);
    let ops = damage_over_time::settlement_rule_ops(managers, owner_uids);
    append_round_phase(
        &mut result,
        drain::run_buff_act_ops(managers, pool, catalog, determinism, context, ops)?,
    );
    let duration = drain::run(
        managers,
        pool,
        catalog,
        determinism,
        context,
        [RuleOp::Command(BattleCommand::Buff(
            BuffCommand::AdvanceDuration(
                BuffDurationAdvance::new(
                    effect_time::ROUND_END_ENTITY_SETTLEMENT,
                    owner_uids.to_vec(),
                    None,
                )
                .expect("round-end settlement effect time is registered"),
            ),
        ))],
    )?;
    append_round_phase(&mut result, duration);
    Ok(result)
}

fn begin_round_phase(phase: RoundPhase) -> DrainResult {
    DrainResult {
        frames: vec![SemanticFrame {
            owner: FrameOwner::RoundPhase(phase),
            trigger: FrameTrigger::Active,
            items: Vec::new(),
        }],
        ..Default::default()
    }
}

fn begin_round_start_event() -> DrainResult {
    DrainResult {
        frames: vec![SemanticFrame {
            owner: FrameOwner::RoundPhase(RoundPhase::RoundStartEvent),
            trigger: FrameTrigger::Active,
            items: vec![FrameItem::Child(Box::new(SemanticFrame {
                owner: FrameOwner::EventRule,
                trigger: FrameTrigger::Event(BattleEvent::RoundStart),
                items: Vec::new(),
            }))],
        }],
        ..Default::default()
    }
}

fn append_round_phase(result: &mut DrainResult, next: DrainResult) {
    append_round_phase_frames(result, next, false);
}

fn append_opening_round_phase(result: &mut DrainResult, next: DrainResult) {
    append_round_phase_frames(result, next, true);
}

fn append_round_phase_frames(
    result: &mut DrainResult,
    next: DrainResult,
    flatten_setup_side: bool,
) {
    result.outcomes.extend(next.outcomes);
    result.events.extend(next.events);
    let root = result.frames.first_mut().expect("round phase has a root");
    let items = if matches!(
        root.owner,
        FrameOwner::RoundPhase(RoundPhase::RoundStartEvent)
    ) {
        let [FrameItem::Child(event)] = root.items.as_mut_slice() else {
            unreachable!("round-start event phase has one event owner")
        };
        &mut event.items
    } else {
        &mut root.items
    };
    let event = matches!(
        root.owner,
        FrameOwner::RoundPhase(RoundPhase::RoundStartEvent)
    )
    .then_some(BattleEvent::RoundStart);
    for frame in next.frames {
        if let Some(event) = event.as_ref() {
            append_to_event(items, frame, event);
        } else if (flatten_setup_side && matches!(frame.owner, FrameOwner::SetupSide(_)))
            || (matches!(frame.owner, FrameOwner::Command)
                && matches!(frame.trigger, FrameTrigger::Active))
        {
            items.extend(frame.items);
        } else {
            items.push(FrameItem::Child(Box::new(frame)));
        }
    }
}

fn append_to_event(items: &mut Vec<FrameItem>, frame: SemanticFrame, event: &BattleEvent) {
    let transparent = (matches!(frame.owner, FrameOwner::Command)
        && matches!(frame.trigger, FrameTrigger::Active))
        || (matches!(frame.owner, FrameOwner::EventRule)
            && matches!(&frame.trigger, FrameTrigger::Event(trigger) if trigger == event))
        || matches!(
            frame.owner,
            FrameOwner::SetupSide(_) | FrameOwner::SetupEntity { .. }
        );
    if !transparent {
        items.push(FrameItem::Child(Box::new(frame)));
        return;
    }
    for item in frame.items {
        match item {
            FrameItem::Child(child) => append_to_event(items, *child, event),
            item => items.push(item),
        }
    }
}

/// Executes registered AI choices through the same skill and manager pipeline.
pub fn run_ai_actions(
    fight: &sonettobuf::Fight,
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    choices: impl IntoIterator<Item = AiSkillChoice>,
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    for (index, choice) in choices.into_iter().enumerate() {
        if crate::engine::round::outcome::battle_ended(fight, pool, managers) {
            break;
        }
        let card_index = index as i32 + 1;
        if managers.hp.current(choice.source_uid) <= 0 {
            push_attributed_cue(
                &mut result.frames,
                choice.source_uid,
                RoundCue::CardInvalid {
                    card_index,
                    team_type: managers
                        .buff
                        .team_type(choice.source_uid)
                        .unwrap_or_default(),
                    reason: CardInvalidReason::Default,
                },
            );
            let is_ultimate = pool.entity(choice.source_uid).is_some_and(|entity| {
                crate::engine::mechanic::card::CardMechanic
                    .is_ultimate_skill(choice.skill_id, entity)
            });
            if let Some(delta) = card_play_resource_delta(
                managers,
                choice.source_uid,
                catalog.grants_resource_on_card_play(choice.skill_id),
                is_ultimate,
            ) {
                append(
                    &mut result,
                    run_card_composition_rewards(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        context,
                        CARD_PLAY_ORIGIN,
                        [(choice.source_uid, delta)],
                    )?,
                );
            }
            continue;
        }
        if card_skill_is_blocked(managers, catalog, choice.source_uid, choice.skill_id) {
            push_attributed_cue(
                &mut result.frames,
                choice.source_uid,
                RoundCue::CardInvalid {
                    card_index,
                    team_type: managers
                        .buff
                        .team_type(choice.source_uid)
                        .unwrap_or_default(),
                    reason: CardInvalidReason::Default,
                },
            );
            continue;
        }
        let mut invocation: crate::engine::skill::action::SkillInvocation =
            crate::engine::skill::action::SkillRequest {
                source_uid: choice.source_uid,
                skill_id: choice.skill_id,
            }
            .into();
        invocation.card_index = card_index;
        if choice.target_uid != 0 {
            invocation.target =
                crate::engine::skill::action::SkillTarget::Explicit(choice.target_uid);
        }
        append(
            &mut result,
            run_active_action(
                managers,
                pool,
                catalog,
                determinism,
                context,
                ActiveActionRequest {
                    skill: invocation,
                    grants_ex_point: true,
                    grant_after_action: false,
                    queued_resource_delta: 0,
                    prelude: Vec::new(),
                },
            )?,
        );
    }
    Ok(result)
}

pub fn run_no_action_round(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    for &owner_uid in owner_uids {
        let mut owner_context = context;
        owner_context.owner_played_card = managers
            .card
            .played()
            .iter()
            .any(|played| played.caster_uid == owner_uid);
        append(
            &mut result,
            drain::run_group_event(
                managers,
                pool,
                catalog,
                determinism,
                owner_context,
                BattleEvent::Kind(EventKind::NoActionRound),
                drain::ReactionLane::Skills,
                Some(std::slice::from_ref(&owner_uid)),
            )?,
        );
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
pub fn run_promotions(
    fight: &sonettobuf::Fight,
    managers: &mut BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    promotions: impl IntoIterator<Item = crate::engine::fight::reserve::Promotion>,
) -> Result<DrainResult, DrainError> {
    let pool = TargetPool::from_fight(fight);
    let mut result = DrainResult::default();
    for promotion in promotions {
        let entering_uid = promotion.entering_uid;
        let origin = CommandOrigin {
            domain: RuleDomain::Lifecycle,
            key: DefinitionKey::new(0, "ReservePromotion"),
        };
        let mut promoted = drain::run_command_group(
            managers,
            &pool,
            catalog,
            determinism,
            context,
            [RuleOp::Command(BattleCommand::Card(
                CardCommand::RemoveAiOwner(crate::engine::manager::card::CardRemoveAiOwner {
                    origin,
                    owner_uid: promotion.defeated_uid,
                    team_type: promotion.team_type,
                }),
            ))],
        )?;
        let composed_owners = promoted
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome {
                RuleOutcome::Card(changes) => Some(changes.composed_owners.as_slice()),
                _ => None,
            })
            .flatten()
            .copied()
            .filter(|owner_uid| managers.ex_point.gains_composition_ex_point(*owner_uid))
            .collect::<Vec<_>>();
        let mut rewards = run_card_composition_rewards(
            managers,
            &pool,
            catalog,
            determinism,
            context,
            origin,
            composed_owners.into_iter().map(|owner_uid| (owner_uid, 1)),
        )?;
        promoted.outcomes.append(&mut rewards.outcomes);
        promoted.events.append(&mut rewards.events);
        promoted
            .frames
            .first_mut()
            .expect("a promotion command group owns one root frame")
            .items
            .extend(
                rewards
                    .frames
                    .pop()
                    .expect("composition rewards own one root frame")
                    .items,
            );
        debug_assert!(rewards.frames.is_empty());
        let command_items = promoted
            .frames
            .pop()
            .expect("a promotion command group owns one root frame")
            .items;
        debug_assert!(promoted.frames.is_empty());
        let mut items = command_items;
        items.push(FrameItem::Change(Box::new(BattleChange::EntityPromotion(
            Box::new(promotion),
        ))));
        let entered = drain::run_owner_event(
            managers,
            &pool,
            catalog,
            determinism,
            context,
            BattleEvent::EntityEntered {
                target_uid: entering_uid,
            },
            std::slice::from_ref(&entering_uid),
        )?;
        items.extend(
            entered
                .frames
                .into_iter()
                .map(|frame| FrameItem::Child(Box::new(frame))),
        );
        promoted.frames.push(SemanticFrame {
            owner: FrameOwner::Command,
            trigger: FrameTrigger::Active,
            items,
        });
        promoted.outcomes.extend(entered.outcomes);
        promoted.events.extend(entered.events);
        append(&mut result, promoted);
    }
    Ok(result)
}

pub fn run_wave_entry_setup(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    let mut result = begin_round_phase(RoundPhase::EntityEntrySetup);
    let next = drain::run_setup_stage_for_owners(
        managers,
        pool,
        catalog,
        determinism,
        context,
        SetupStage::EnterFight,
        0,
        owner_uids,
    )?;
    result.outcomes.extend(next.outcomes);
    result.events.extend(next.events);
    let root = result
        .frames
        .first_mut()
        .expect("entry-setup phase has a root");
    for frame in next.frames {
        if matches!(frame.owner, FrameOwner::SetupSide(_)) {
            root.items.extend(frame.items);
        } else {
            root.items.push(FrameItem::Child(Box::new(frame)));
        }
    }
    Ok(result)
}

pub fn run_wave_entry(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    change: crate::engine::manager::wave::WaveAdvanced,
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    let root = crate::engine::runtime::record::push_root(
        &mut result.frames,
        crate::engine::runtime::record::FrameOwner::StageWave { wave: change.wave },
        crate::engine::runtime::record::FrameTrigger::Active,
    );
    crate::engine::runtime::record::push_change(
        &mut result.frames,
        &root,
        BattleChange::WaveAdvanced(Box::new(change.clone())),
    );
    for &entering_uid in &change.entering_uids {
        append(
            &mut result,
            drain::run_owner_event(
                managers,
                pool,
                catalog,
                determinism,
                context,
                BattleEvent::EntityEntered {
                    target_uid: entering_uid,
                },
                std::slice::from_ref(&entering_uid),
            )?,
        );
    }
    Ok(result)
}

fn append(result: &mut DrainResult, next: DrainResult) {
    result.outcomes.extend(next.outcomes);
    result.events.extend(next.events);
    result.frames.extend(next.frames);
}

#[cfg(test)]
mod tests;
