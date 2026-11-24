mod ai;
mod draw;
mod energy;
mod hand;
mod queue;
mod sovereign;
mod temporary;

use super::*;

pub(in crate::engine::manager::card) fn execute(
    manager: &mut CardManager,
    command: CardCommand,
) -> Result<CardChanges, CardCommandError> {
    let before = manager.hand().to_vec();
    let mut changes = match command {
        CardCommand::Setup(setup) => draw::setup(manager, setup)?,
        CardCommand::PreserveRefillFloor => draw::preserve_refill_floor(manager),
        CardCommand::PreserveDeckCount => draw::preserve_deck_count(manager),
        CardCommand::ConsumeForEffect(consume) => hand::consume_for_effect(manager, consume)?,
        CardCommand::Move {
            origin,
            from_index,
            to_index,
        } => hand::move_card(manager, origin, from_index, to_index)?,
        CardCommand::UseUniversal(use_card) => hand::use_universal(manager, use_card)?,
        CardCommand::Dissolve { origin, card_index } => {
            hand::dissolve(manager, origin, card_index)?
        }
        CardCommand::InitialSovereignBoost(boost) => sovereign::initial_boost(manager, boost)?,
        CardCommand::ShiftSovereign {
            origin,
            owner_uid,
            distance,
            team_type,
        } => sovereign::shift(manager, origin, owner_uid, distance, team_type)?,
        CardCommand::UnlockSovereign {
            origin,
            owner_uid,
            team_type,
        } => sovereign::unlock(manager, origin, owner_uid, team_type)?,
        CardCommand::UnlockHedone(unlock) => hand::unlock_hedone(manager, unlock)?,
        CardCommand::ChargeSovereign {
            origin,
            owner_uid,
            kind,
            team_type,
        } => sovereign::charge(manager, origin, owner_uid, kind, team_type)?,
        CardCommand::InsertSovereign(insert) => sovereign::insert(manager, insert)?,
        CardCommand::AddGenerated(add) => hand::add_generated(manager, add)?,
        CardCommand::AddHedone(add) => hand::add_hedone(manager, add)?,
        CardCommand::AddUniversal(add) => hand::add_universal(manager, add)?,
        CardCommand::RedealKeepRanks { .. } => return Err(CardCommandError::InvalidCommand),
        CardCommand::ApplyRedealKeepRanks(redeal) => {
            draw::apply_redeal_keep_ranks(manager, redeal)?
        }
        CardCommand::AddTemporary(add) => temporary::add_temporary(manager, add)?,
        CardCommand::AddCrystal(add) => temporary::add_crystal(manager, add)?,
        CardCommand::AddPrecast(add) => temporary::add_precast(manager, add)?,
        CardCommand::AddSelectedPrecast(add) => temporary::add_selected_precast(manager, add)?,
        CardCommand::ChangeToTemporary(change) => temporary::change_to_temporary(manager, change)?,
        CardCommand::EnchantHand(change) => hand::enchant_hand(manager, change)?,
        CardCommand::MarkTemporary(change) => temporary::mark_temporary(manager, change)?,
        CardCommand::PersistTemporary(change) => temporary::persist_temporary(manager, change)?,
        CardCommand::ExpireTemporary { origin } => temporary::expire_temporary(manager, origin),
        CardCommand::Play(play) => hand::play(manager, play)?,
        CardCommand::Draw(draw) => draw::draw(manager, draw)?,
        CardCommand::DealOpening(draw) => draw::deal_opening(manager, draw)?,
        CardCommand::RecycleDrawPile { origin, team_type } => {
            draw::recycle_draw_pile(manager, origin, team_type)?
        }
        CardCommand::RefillOne(refill) => draw::refill_one(manager, refill)?,
        CardCommand::SetUltimateAvailability(set) => hand::set_ultimate_availability(manager, set)?,
        CardCommand::PrepareAiSkill(prepare) => ai::prepare_ai_skill(manager, prepare)?,
        CardCommand::RefreshAiQueue(refresh) => ai::refresh_ai_queue(manager, refresh),
        CardCommand::SetAiQueue(set) => ai::set_ai_queue(manager, set),
        CardCommand::RecordAiCast(record) => ai::record_ai_cast(manager, record),
        CardCommand::SetTeamCards(set) => ai::set_team_cards(manager, set),
        CardCommand::RemoveOwner(remove) => hand::remove_owner(manager, remove)?,
        CardCommand::InvalidateAiOwner(invalidate) => ai::invalidate_ai_owner(manager, invalidate)?,
        CardCommand::ReplaceOwnerSkills(replace) => hand::replace_owner_skills(manager, replace)?,
        CardCommand::InvalidatePlayed(invalid) => queue::invalidate_played(manager, invalid)?,
        CardCommand::ComposeAdjacent { origin } => hand::compose_adjacent(manager, origin),
        CardCommand::AllocateEnergy(allocation) => energy::allocate_energy(manager, allocation)?,
        CardCommand::ChangeBasicEnergy(change) => energy::change_basic_energy(manager, change)?,
        CardCommand::ClearEnergy { origin } => energy::clear_energy(manager, origin),
        CardCommand::ChangeHandLimit(change) => hand::change_hand_limit(manager, change)?,
        CardCommand::ClearHandLimit { origin } => hand::clear_hand_limit(manager, origin),
        CardCommand::RankUpQueued {
            origin,
            after_card_index,
            count,
            levels,
        } => queue::rank_up_queued(manager, origin, after_card_index, count, levels)?,
        CardCommand::RewriteQueuedRanks { origin, upgrades } => {
            queue::rewrite_queued_ranks(manager, origin, upgrades)?
        }
        CardCommand::ChangeQueuedRanks { origin, changes } => {
            queue::change_queued_ranks(manager, origin, changes)?
        }
        CardCommand::RankUpHand(change) => hand::rank_up_hand(manager, change)?,
        CardCommand::RankUpDeckRange(change) => draw::rank_up_deck_range(manager, change)?,
        CardCommand::CompleteActionPhase { origin } => {
            queue::complete_action_phase(manager, origin)
        }
        CardCommand::BeginPostActionRefill { origin } => {
            draw::begin_post_action_refill(manager, origin)
        }
        CardCommand::ResolvePlayedRanks { origin } => queue::resolve_played_ranks(manager, origin),
        CardCommand::RecordCastChannel(record) => queue::record_cast_channel(manager, record)?,
        CardCommand::ResolveCastChannel { origin, buff_uid } => {
            queue::resolve_cast_channel(manager, origin, buff_uid)?
        }
        CardCommand::RemoveCastChannel { origin, buff_uid } => {
            queue::remove_cast_channel(manager, origin, buff_uid)?
        }
        CardCommand::QueueUseCard(queue) => queue::queue_use_card(manager, queue)?,
    };
    let after = manager.hand().to_vec();
    manager.record_opening_deal_change(
        changes.kind,
        &before,
        &after,
        changes.added.as_ref(),
        &changes.drawn,
    );
    changes.before = before;
    changes.after = after;
    Ok(changes)
}

impl CardChanges {
    fn new(origin: Option<CommandOrigin>, kind: CardChangeKind) -> Self {
        Self {
            origin,
            kind,
            before: Vec::new(),
            after: Vec::new(),
            added: None,
            played: None,
            drawn: Vec::new(),
            composed_owners: Vec::new(),
            operation: None,
            queued_use_card: None,
            rank_results: Vec::new(),
            consumed_indices: Vec::new(),
            expired_indices: Vec::new(),
            consumed_owner_uid: None,
            consumption_kind: None,
            hand_limit: None,
            owner_removal: None,
            sovereign_boost: None,
            ai_invalidations: Vec::new(),
            ai_queue: None,
            team_cards: None,
            entity: None,
        }
    }
}
