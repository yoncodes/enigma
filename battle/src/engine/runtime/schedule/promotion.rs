use super::*;
use crate::engine::{
    fight::reserve::{Promotion, PromotionBatch},
    mechanic::reserve_cycle::{self, Window},
};

pub(crate) fn sync_cycle_roster(
    fight: &mut sonettobuf::Fight,
    managers: &mut BattleManagers,
    pool: &mut TargetPool,
    actions: &DrainResult,
) {
    if actions
        .outcomes
        .iter()
        .any(|outcome| matches!(outcome, RuleOutcome::ReserveCycleCommitted))
        && pool.roster_changed(managers)
    {
        managers.sync_entities(fight);
        managers.sync_roster(fight);
        *pool = TargetPool::from_fight_with_catalog(pool.catalog(), fight);
    }
}

pub(super) fn run_cycles(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> Result<DrainResult, DrainError> {
    use crate::engine::fight::versions::{RoundStartSetupLayout, round_start_setup_layout};

    if round_start_setup_layout(managers.fight_version()) != Some(RoundStartSetupLayout::Version7)
        || crate::engine::round::outcome::battle_ended_for_battle_id(
            context.battle_id,
            pool,
            managers,
        )
    {
        return Ok(DrainResult::default());
    }
    let promotions = reserve_cycle::promote(managers, Window::AfterAction);
    let Some(team_type) = promotions
        .promotions()
        .first()
        .map(|promotion| promotion.team_type)
    else {
        return Ok(DrainResult::default());
    };
    let mut result = run_cycles_batch(
        pool,
        managers,
        catalog,
        determinism,
        context,
        &promotions,
        team_type,
    )?;
    result.outcomes.push(RuleOutcome::ReserveCycleCommitted);
    Ok(result)
}

pub fn run_promotions(
    pool: &TargetPool,
    managers: &mut BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    promotions: &PromotionBatch,
) -> Result<DrainResult, DrainError> {
    let pool = pool.runtime_view(managers);
    let mut result = DrainResult::default();
    for promotion in promotions.promotions() {
        let cards = remove_cards(&pool, managers, catalog, determinism, context, promotion)?;
        let entered = settle_entry(
            &pool,
            managers,
            catalog,
            determinism,
            context,
            promotion,
            cards,
        )?;
        append(&mut result, entered);
    }
    retire(managers, promotions, &mut result)?;
    Ok(result)
}

fn run_cycles_batch(
    pool: &TargetPool,
    managers: &mut BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    promotions: &PromotionBatch,
    team_type: i32,
) -> Result<DrainResult, DrainError> {
    let pool = pool.runtime_view(managers);
    let mut phase = begin_round_phase(RoundPhase::ReserveCycle);
    phase.frames[0]
        .items
        .push(FrameItem::Cue(RoundCue::ReserveSplitStart { team_type }));
    for promotion in promotions.promotions() {
        let cleanup = remove_cards(&pool, managers, catalog, determinism, context, promotion)?;
        phase.outcomes.extend(cleanup.outcomes);
        phase.events.extend(cleanup.events);
        phase.frames[0].items.extend(
            cleanup
                .frames
                .into_iter()
                .map(|frame| FrameItem::Child(Box::new(frame))),
        );
    }
    for promotion in promotions.promotions() {
        let entered = settle_entry(
            &pool,
            managers,
            catalog,
            determinism,
            context,
            promotion,
            DrainResult::default(),
        )?;
        append_round_phase(&mut phase, entered);
    }
    retire(managers, promotions, &mut phase)?;
    phase.frames[0]
        .items
        .push(FrameItem::Cue(RoundCue::ReserveSplitEnd { team_type }));
    Ok(phase)
}

fn remove_cards(
    pool: &TargetPool,
    managers: &mut BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    promotion: &Promotion,
) -> Result<DrainResult, DrainError> {
    let origin = CommandOrigin {
        domain: RuleDomain::Lifecycle,
        key: DefinitionKey::new(0, "ReservePromotion"),
    };
    let mut promoted = drain::run_command_group(
        managers,
        pool,
        catalog,
        determinism,
        context,
        [RuleOp::Command(BattleCommand::Card(
            CardCommand::remove_owner(crate::engine::manager::card::CardRemoveOwner {
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
        pool,
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
    Ok(promoted)
}

fn settle_entry(
    pool: &TargetPool,
    managers: &mut BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    promotion: &Promotion,
    mut promoted: DrainResult,
) -> Result<DrainResult, DrainError> {
    let mut promotion = promotion.clone();
    let entering_uid = promotion.entering_uid;
    let mut items = promoted
        .frames
        .drain(..)
        .map(|frame| FrameItem::Child(Box::new(frame)))
        .collect::<Vec<_>>();
    let queued = promotion.queued_reserve.take();
    items.push(FrameItem::Change(Box::new(BattleChange::EntityPromotion(
        Box::new(promotion),
    ))));
    if let Some(queued) = queued {
        items.push(FrameItem::Change(Box::new(BattleChange::ReserveQueued(
            Box::new(queued),
        ))));
    }
    let entered = drain::run_owner_event(
        managers,
        pool,
        catalog,
        determinism,
        context,
        BattleEvent::EntityEntered {
            target_uid: entering_uid,
            entry: EntityEntry::Reserve,
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
    Ok(promoted)
}

fn retire(
    managers: &mut BattleManagers,
    promotions: &PromotionBatch,
    result: &mut DrainResult,
) -> Result<(), DrainError> {
    for promotion in promotions
        .promotions()
        .iter()
        .filter(|promotion| promotion.queued_reserve.is_some())
    {
        let retired = managers
            .entity
            .retire_reserve(promotion.defeated_uid)
            .map_err(|error| DrainError::Command(error.into()))?;
        result
            .frames
            .last_mut()
            .expect("reserve retirement follows a promotion")
            .items
            .push(FrameItem::Change(Box::new(BattleChange::ReserveRetired(
                retired,
            ))));
    }
    Ok(())
}
