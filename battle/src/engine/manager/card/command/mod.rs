use sonettobuf::{CardInfo, FightEntityInfo};

use crate::engine::skill::rule::{DefinitionKey, RuleDomain};
use crate::engine::{event::payload::BattleEvent, skill::rule::CommandOrigin};

use super::{CardChange, CardManager, CardPlayChoice, EnchantedType, PlayedCard};

pub const CARD_PLAY_ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Lifecycle,
    key: DefinitionKey::new(0, "CardPlay"),
};

pub const CARD_ENERGY_CLEAR_ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Lifecycle,
    key: DefinitionKey::new(0, "PlayerActionsResolvedCardEnergy"),
};

#[derive(Debug, Clone, PartialEq)]
pub struct CardSetup {
    pub hand: Vec<CardInfo>,
    pub draw_pile: Vec<CardInfo>,
    pub deck_num: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardAddTemporary {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub skill_id: i32,
    pub reserve_id: i64,
    pub team_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardAddGenerated {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub skill_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardAddUniversal {
    pub origin: CommandOrigin,
    pub count: i32,
    pub rank: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardRedealKeepRanks {
    pub origin: CommandOrigin,
    pub replacements: Vec<CardInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardUseUniversal {
    pub origin: CommandOrigin,
    pub universal_index: usize,
    pub target_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardAddCrystal {
    pub origin: CommandOrigin,
    pub card: CardInfo,
    pub rank_group: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardAddPrecast {
    pub origin: CommandOrigin,
    pub card: CardInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardEnergyChange {
    pub origin: CommandOrigin,
    pub delta: i32,
    pub count: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardHandLimitChange {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub delta: i32,
    pub resulting_limit: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardEnchantHand {
    pub origin: CommandOrigin,
    pub indices: Vec<usize>,
    pub enchant: EnchantedType,
    pub duration: i32,
    pub team_type: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardMarkTemporary {
    pub origin: CommandOrigin,
    pub indices: Vec<usize>,
    pub team_type: i32,
    pub config_effect: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardEnergyAllocation {
    pub origin: CommandOrigin,
    pub energies: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardChangeToTemporary {
    pub origin: CommandOrigin,
    pub index: usize,
    pub skill_id: i32,
    pub target_uid: i64,
    pub reserve: String,
    pub team_type: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardPlay {
    pub origin: CommandOrigin,
    pub hand_index: usize,
    pub target_uid: Option<i64>,
    pub chosen_skill_id: Option<i32>,
    pub choice: Option<CardPlayChoice>,
    pub recorded_skill: Option<crate::engine::skill::action::SkillRequest>,
}

impl CardPlay {
    pub(crate) fn planned_skill(&self, visible: Option<&CardInfo>) -> Option<(i64, i32)> {
        let source = self
            .choice
            .as_ref()
            .map(|choice| &choice.source)
            .or(visible)?;
        let skill_id = self.choice.as_ref().map_or_else(
            || self.chosen_skill_id.or(source.skill_id),
            |choice| choice.played.skill_id,
        )?;
        (skill_id > 0).then_some((source.uid.unwrap_or_default(), skill_id))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardDraw {
    pub origin: CommandOrigin,
    pub count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardOpeningDraw {
    pub origin: CommandOrigin,
    pub cards: Vec<CardInfo>,
    pub deck_cost: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardRefillOne {
    pub origin: CommandOrigin,
    pub card: CardInfo,
    pub consume_draw_pile: bool,
    pub consume_deck: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardRefreshAiQueue {
    pub origin: CommandOrigin,
    pub cards: Vec<CardInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardSetTeamCards {
    pub origin: CommandOrigin,
    pub cards: Vec<CardInfo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardSetAiQueue {
    pub origin: CommandOrigin,
    pub cards: Vec<CardInfo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardRemoveAiOwner {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub team_type: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardReplaceOwnerSkills {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub base_group1: Vec<i32>,
    pub base_group2: Vec<i32>,
    pub replacement_group1: Vec<i32>,
    pub replacement_group2: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardOwnerRemoval {
    pub owner_uid: i64,
    pub team_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardInvalidatePlayed {
    pub origin: CommandOrigin,
    pub card_index: i32,
    pub restore: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardActionQueue {
    pub team: i32,
    pub emitter_uid: i64,
    pub cards: Vec<CardInfo>,
    pub deck_num: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardRankChange {
    pub owner_uid: i64,
    pub card_index: i32,
    pub card: CardInfo,
    pub rewritten: bool,
    pub rank_delta: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardRankFailure {
    pub owner_uid: i64,
    pub card_index: i32,
    pub card: Box<CardInfo>,
    pub requested_delta: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CardRankResult {
    Changed(Box<CardRankChange>),
    Failed(CardRankFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueuedCardRankUp {
    pub card_index: i32,
    pub levels: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueuedCardRankChange {
    pub card_index: i32,
    pub levels: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandCardRankUp {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub hand_index: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardQueueUse {
    pub origin: CommandOrigin,
    pub card_index: i32,
    pub card: CardInfo,
    pub team_type: i32,
    pub source_skill_id: i32,
    pub action: Option<crate::engine::skill::action::SkillInvocation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueuedUseCard {
    pub card_index: i32,
    pub card: CardInfo,
    pub team_type: i32,
    pub source_skill_id: i32,
    pub action: Option<crate::engine::skill::action::SkillInvocation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardRecordCastChannel {
    pub origin: CommandOrigin,
    pub buff_uid: i64,
    pub cards: Vec<CardInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardConsumeForEffect {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum CardCommand {
    Setup(CardSetup),
    PreserveRefillFloor,
    ConsumeForEffect(CardConsumeForEffect),
    Move {
        origin: CommandOrigin,
        from_index: usize,
        to_index: usize,
    },
    UseUniversal(CardUseUniversal),
    Dissolve {
        origin: CommandOrigin,
        card_index: usize,
    },
    AddGenerated(CardAddGenerated),
    AddUniversal(CardAddUniversal),
    RedealKeepRanks {
        origin: CommandOrigin,
    },
    ApplyRedealKeepRanks(CardRedealKeepRanks),
    AddTemporary(CardAddTemporary),
    AddCrystal(CardAddCrystal),
    AddPrecast(CardAddPrecast),
    AddSelectedPrecast(CardAddPrecast),
    ChangeToTemporary(CardChangeToTemporary),
    EnchantHand(CardEnchantHand),
    MarkTemporary(CardMarkTemporary),
    ExpireTemporary {
        origin: CommandOrigin,
    },
    Play(CardPlay),
    Draw(CardDraw),
    DealOpening(CardOpeningDraw),
    RecycleDrawPile {
        origin: CommandOrigin,
        team_type: i32,
    },
    RefillOne(CardRefillOne),
    RefreshAiQueue(CardRefreshAiQueue),
    SetAiQueue(CardSetAiQueue),
    SetTeamCards(CardSetTeamCards),
    RemoveAiOwner(CardRemoveAiOwner),
    ReplaceOwnerSkills(CardReplaceOwnerSkills),
    InvalidatePlayed(CardInvalidatePlayed),
    ComposeAdjacent {
        origin: CommandOrigin,
    },
    AllocateEnergy(CardEnergyAllocation),
    ChangeBasicEnergy(CardEnergyChange),
    ClearEnergy {
        origin: CommandOrigin,
    },
    ChangeHandLimit(CardHandLimitChange),
    ClearHandLimit {
        origin: CommandOrigin,
    },
    RankUpQueued {
        origin: CommandOrigin,
        after_card_index: i32,
        count: i32,
        levels: i32,
    },
    RankUpQueuedCards {
        origin: CommandOrigin,
        upgrades: Vec<QueuedCardRankUp>,
        rewritten: bool,
    },
    ChangeAroundQueuedRanks {
        origin: CommandOrigin,
        changes: Vec<QueuedCardRankChange>,
    },
    RankUpHand(HandCardRankUp),
    CommitActionQueue {
        team: i32,
        emitter_uid: i64,
    },
    ResolvePlayedRanks {
        origin: CommandOrigin,
    },
    RecordCastChannel(CardRecordCastChannel),
    ResolveCastChannel {
        origin: CommandOrigin,
        buff_uid: i64,
    },
    RemoveCastChannel {
        origin: CommandOrigin,
        buff_uid: i64,
    },
    QueueUseCard(CardQueueUse),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardChangeKind {
    Setup,
    ConsumedForEffect,
    Moved,
    Dissolved,
    GeneratedAdded,
    UniversalAdded,
    RedealtKeepRanks,
    TemporaryAdded,
    CrystalAdded,
    PrecastAdded,
    TemporaryChanged,
    Enchanted,
    TemporaryExpired,
    Played,
    Drawn,
    OpeningDrawn,
    DrawPileRecycled,
    Refilled,
    AiQueueRefreshed,
    AiQueueSet,
    TeamCardsSet,
    AiOwnerRemoved,
    OwnerSkillsReplaced,
    PlayedInvalidated,
    Composed,
    EnergyAllocated,
    EnergyChanged,
    EnergyCleared,
    HandLimitChanged,
    HandLimitCleared,
    QueuedRankChanged,
    AroundRanksChanged,
    HandRankChanged,
    ActionQueueCommitted,
    PlayedRanksResolved,
    CastChannelRecorded,
    CastChannelResolved,
    CastChannelRemoved,
    UseCardQueued,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardChanges {
    pub origin: Option<CommandOrigin>,
    pub kind: CardChangeKind,
    pub before: Vec<CardInfo>,
    pub after: Vec<CardInfo>,
    pub added: Option<CardInfo>,
    pub played: Option<PlayedCard>,
    pub drawn: Vec<CardInfo>,
    pub composed_owners: Vec<i64>,
    pub operation: Option<CardChange>,
    pub action_queue: Option<CardActionQueue>,
    pub queued_use_card: Option<QueuedUseCard>,
    pub rank_results: Vec<CardRankResult>,
    pub consumed_indices: Vec<usize>,
    pub entity: Option<FightEntityInfo>,
    pub hand_limit: Option<(i64, i32)>,
    pub owner_removal: Option<CardOwnerRemoval>,
    pub ai_queue: Option<Vec<CardInfo>>,
    pub team_cards: Option<Vec<CardInfo>>,
}

impl CardChanges {
    pub fn events(&self) -> Vec<BattleEvent> {
        if let Some(queue) = &self.action_queue {
            return vec![BattleEvent::ActionQueueCommitted {
                team: queue.team,
                emitter_uid: queue.emitter_uid,
                cards: queue.cards.clone(),
            }];
        }
        self.origin
            .filter(|_| self.before != self.after)
            .map(|origin| {
                BattleEvent::CardChanged(crate::engine::event::payload::CardChangeEvent {
                    origin,
                    kind: self.kind,
                    before_count: self.before.len() as i32,
                    after_count: self.after.len() as i32,
                    before_energy: total_energy(&self.before),
                    after_energy: total_energy(&self.after),
                })
            })
            .into_iter()
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardCommandError {
    InvalidCommand,
    InvalidPlaySkill,
    InvalidPlayIndex,
    PlayedCardNotFound,
}

pub(super) fn execute(
    manager: &mut CardManager,
    command: CardCommand,
) -> Result<CardChanges, CardCommandError> {
    let before = manager.hand().to_vec();
    let mut operation = None;
    let mut action_queue = None;
    let mut queued_use_card = None;
    let mut rank_results = Vec::new();
    let mut consumed_indices = Vec::new();
    let mut hand_limit = None;
    let mut owner_removal = None;
    let mut ai_queue = None;
    let mut team_cards = None;
    let (origin, kind, added, played, drawn, composed_owners) = match command {
        CardCommand::Setup(setup) => {
            if setup.deck_num < 0
                || setup
                    .draw_pile
                    .iter()
                    .any(|card| card.temp_card.unwrap_or_default())
            {
                return Err(CardCommandError::InvalidCommand);
            }
            manager.reset_with_draw_pile(setup.hand, setup.draw_pile, setup.deck_num);
            (
                None,
                CardChangeKind::Setup,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::PreserveRefillFloor => {
            manager.preserve_refill_floor();
            (
                None,
                CardChangeKind::Setup,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::ConsumeForEffect(consume) => {
            if !manager.consume_for_effect(consume.owner_uid, &consume.indices) {
                return Err(CardCommandError::InvalidCommand);
            }
            consumed_indices = consume.indices;
            (
                Some(consume.origin),
                CardChangeKind::ConsumedForEffect,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::Move {
            origin,
            from_index,
            to_index,
        } => {
            if !manager.move_card(from_index, to_index) {
                return Err(CardCommandError::InvalidCommand);
            }
            (
                Some(origin),
                CardChangeKind::Moved,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::UseUniversal(use_card) => {
            let owner_uid = manager
                .use_universal(use_card.universal_index, use_card.target_index)
                .ok_or(CardCommandError::InvalidCommand)?;
            (
                Some(use_card.origin),
                CardChangeKind::Composed,
                None,
                None,
                Vec::new(),
                vec![owner_uid],
            )
        }
        CardCommand::Dissolve { origin, card_index } => {
            manager
                .dissolve(card_index, None)
                .ok_or(CardCommandError::InvalidCommand)?;
            operation = Some(CardChange::CardsPush {
                cards: manager.hand().to_vec(),
                team_type: 1,
            });
            (
                Some(origin),
                CardChangeKind::Dissolved,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::AddGenerated(add) => {
            if add.target_uid == 0 || add.skill_id <= 0 {
                return Err(CardCommandError::InvalidCommand);
            }
            let card = manager.add_to_hand_for(
                add.target_uid,
                CardInfo {
                    uid: Some(add.target_uid),
                    skill_id: Some(add.skill_id),
                    temp_card: Some(false),
                    ..Default::default()
                },
            );
            operation = Some(CardChange::AddHand {
                target_uid: add.target_uid,
                card: card.clone(),
            });
            (
                Some(add.origin),
                CardChangeKind::GeneratedAdded,
                Some(card),
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::AddUniversal(add) => {
            let Some(skill) = super::UniversalCardSkill::from_rank(add.rank) else {
                return Err(CardCommandError::InvalidCommand);
            };
            if add.count <= 0 {
                return Err(CardCommandError::InvalidCommand);
            }
            manager.add_universal_cards(add.count, skill);
            (
                Some(add.origin),
                CardChangeKind::UniversalAdded,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::RedealKeepRanks { .. } => return Err(CardCommandError::InvalidCommand),
        CardCommand::ApplyRedealKeepRanks(redeal) => {
            if redeal.replacements.len() != manager.redealable_count() {
                return Err(CardCommandError::InvalidCommand);
            }
            manager.redeal_keep_ranks(redeal.replacements);
            (
                Some(redeal.origin),
                CardChangeKind::RedealtKeepRanks,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::AddTemporary(add) => {
            if add.skill_id <= 0 || add.team_type == 0 {
                return Err(CardCommandError::InvalidCommand);
            }
            let card = manager.add_temp_card_for(
                add.target_uid,
                add.skill_id,
                add.reserve_id,
                add.team_type,
            );
            operation = Some(CardChange::SpCardAdd {
                target_uid: add.target_uid,
                skill_id: add.skill_id,
                reserve_id: add.reserve_id,
                team_type: add.team_type,
            });
            (
                Some(add.origin),
                CardChangeKind::TemporaryAdded,
                Some(card),
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::AddCrystal(add) => {
            if add.card.uid.unwrap_or_default() == 0
                || add.card.skill_id.unwrap_or_default() <= 0
                || !add.card.temp_card.unwrap_or_default()
            {
                return Err(CardCommandError::InvalidCommand);
            }
            manager.register_skill_group(add.card.uid.unwrap_or_default(), &add.rank_group);
            let card = manager.add_to_hand(add.card);
            (
                Some(add.origin),
                CardChangeKind::CrystalAdded,
                Some(card),
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::AddPrecast(add) => {
            if add.card.uid.unwrap_or_default() == 0
                || add.card.skill_id.unwrap_or_default() <= 0
                || !add.card.temp_card.unwrap_or_default()
            {
                return Err(CardCommandError::InvalidCommand);
            }
            let card = manager.add_to_hand(add.card);
            (
                Some(add.origin),
                CardChangeKind::PrecastAdded,
                Some(card),
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::AddSelectedPrecast(add) => {
            if add.card.uid.unwrap_or_default() == 0
                || add.card.skill_id.unwrap_or_default() <= 0
                || !add.card.temp_card.unwrap_or_default()
            {
                return Err(CardCommandError::InvalidCommand);
            }
            let target_uid = add.card.uid.unwrap_or_default();
            let card = manager.add_to_hand(add.card);
            operation = Some(CardChange::AddHand {
                target_uid,
                card: card.clone(),
            });
            (
                Some(add.origin),
                CardChangeKind::GeneratedAdded,
                Some(card),
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::ChangeToTemporary(change) => {
            if change.skill_id <= 0 || change.team_type == 0 || change.index >= manager.hand().len()
            {
                return Err(CardCommandError::InvalidCommand);
            }
            let card = manager
                .change_to_temp_card_for(
                    change.index,
                    change.skill_id,
                    change.target_uid,
                    change.reserve.clone(),
                    change.team_type,
                )
                .ok_or(CardCommandError::InvalidCommand)?;
            operation = Some(CardChange::ChangeToTemp {
                target_uid: change.target_uid,
                reserve_str: change.reserve,
                team_type: change.team_type,
            });
            (
                Some(change.origin),
                CardChangeKind::TemporaryChanged,
                Some(card),
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::EnchantHand(change) => {
            let cards = manager
                .enchant_hand(&change.indices, change.enchant, change.duration)
                .ok_or(CardCommandError::InvalidCommand)?;
            operation = Some(CardChange::Enchant {
                cards,
                indices: change.indices,
                team_type: change.team_type,
            });
            (
                Some(change.origin),
                CardChangeKind::Enchanted,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::MarkTemporary(change) => {
            if !manager.mark_hand_temporary(&change.indices) {
                return Err(CardCommandError::InvalidCommand);
            }
            operation = Some(CardChange::MarkTemporary {
                indices: change.indices,
                team_type: change.team_type,
                config_effect: change.config_effect,
            });
            (
                Some(change.origin),
                CardChangeKind::TemporaryChanged,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::ExpireTemporary { origin } => {
            manager.expire_temporary();
            (
                Some(origin),
                CardChangeKind::TemporaryExpired,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::Play(play) => {
            if play.choice.is_none() && manager.visible_card(play.hand_index).is_none() {
                return Err(CardCommandError::InvalidPlayIndex);
            }
            if play
                .planned_skill(manager.visible_card(play.hand_index))
                .is_none()
            {
                return Err(CardCommandError::InvalidPlaySkill);
            }
            let played = manager
                .play_card_with_record(
                    play.hand_index,
                    play.target_uid,
                    play.chosen_skill_id,
                    play.choice,
                    play.recorded_skill,
                )
                .ok_or(CardCommandError::PlayedCardNotFound)?;
            (
                Some(play.origin),
                CardChangeKind::Played,
                None,
                Some(played),
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::Draw(draw) => {
            if draw.count == 0 {
                return Err(CardCommandError::InvalidCommand);
            }
            let cards = manager.draw(draw.count);
            if !cards.is_empty() {
                operation = Some(CardChange::CardsPush {
                    cards: manager.hand().to_vec(),
                    team_type: 1,
                });
            }
            (
                Some(draw.origin),
                CardChangeKind::Drawn,
                None,
                None,
                cards,
                Vec::new(),
            )
        }
        CardCommand::DealOpening(draw) => {
            if draw.cards.is_empty() || !manager.deal_opening_cards(&draw.cards, draw.deck_cost) {
                return Err(CardCommandError::InvalidCommand);
            }
            (
                Some(draw.origin),
                CardChangeKind::OpeningDrawn,
                None,
                None,
                draw.cards,
                Vec::new(),
            )
        }
        CardCommand::RecycleDrawPile { origin, team_type } => {
            if team_type == 0 {
                return Err(CardCommandError::InvalidCommand);
            }
            let deck_num = manager
                .recycle_draw_pile()
                .ok_or(CardCommandError::InvalidCommand)?;
            operation = Some(CardChange::DeckCount {
                deck_num,
                team_type,
            });
            (
                Some(origin),
                CardChangeKind::DrawPileRecycled,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::RefillOne(refill) => {
            if refill.card.skill_id.unwrap_or_default() <= 0 {
                return Err(CardCommandError::InvalidCommand);
            }
            if refill.consume_draw_pile && !manager.consume_draw_card(&refill.card) {
                return Err(CardCommandError::InvalidCommand);
            }
            manager.add_to_hand(refill.card.clone());
            manager.record_refill(refill.card.clone());
            let owners = manager.compose_adjacent();
            if refill.consume_deck {
                manager.set_deck_num(manager.deck_num().saturating_sub(1));
            }
            (
                Some(refill.origin),
                CardChangeKind::Refilled,
                None,
                None,
                vec![refill.card],
                owners,
            )
        }
        CardCommand::RefreshAiQueue(refresh) => {
            let owners = manager.refresh_ai_queue(refresh.cards);
            (
                Some(refresh.origin),
                CardChangeKind::AiQueueRefreshed,
                None,
                None,
                Vec::new(),
                owners,
            )
        }
        CardCommand::SetAiQueue(set) => {
            manager.set_ai_queue(set.cards);
            ai_queue = Some(manager.ai_queue().to_vec());
            (
                Some(set.origin),
                CardChangeKind::AiQueueSet,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::SetTeamCards(set) => {
            manager.set_team_cards(set.cards);
            team_cards = Some(manager.team_cards().to_vec());
            (
                Some(set.origin),
                CardChangeKind::TeamCardsSet,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::RemoveAiOwner(remove) => {
            if remove.owner_uid == 0 || remove.team_type == 0 {
                return Err(CardCommandError::InvalidCommand);
            }
            let owners = manager.remove_ai_owner_cards(remove.owner_uid);
            if owners.is_some() {
                owner_removal = Some(CardOwnerRemoval {
                    owner_uid: remove.owner_uid,
                    team_type: remove.team_type,
                });
            }
            (
                Some(remove.origin),
                CardChangeKind::AiOwnerRemoved,
                None,
                None,
                Vec::new(),
                owners.unwrap_or_default(),
            )
        }
        CardCommand::ReplaceOwnerSkills(replace) => {
            if replace.owner_uid == 0 {
                return Err(CardCommandError::InvalidCommand);
            }
            manager.replace_owner_skills(
                replace.owner_uid,
                &replace.base_group1,
                &replace.base_group2,
                &replace.replacement_group1,
                &replace.replacement_group2,
            );
            (
                Some(replace.origin),
                CardChangeKind::OwnerSkillsReplaced,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::InvalidatePlayed(invalid) => {
            let card = manager
                .invalidate_played(invalid.card_index, invalid.restore)
                .ok_or(CardCommandError::PlayedCardNotFound)?;
            if invalid.restore {
                operation = Some(CardChange::AddHand {
                    target_uid: card.uid.unwrap_or_default(),
                    card: card.clone(),
                });
            }
            (
                Some(invalid.origin),
                CardChangeKind::PlayedInvalidated,
                invalid.restore.then_some(card),
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::ComposeAdjacent { origin } => {
            let owners = manager.compose_adjacent();
            (
                Some(origin),
                CardChangeKind::Composed,
                None,
                None,
                Vec::new(),
                owners,
            )
        }
        CardCommand::AllocateEnergy(allocation) => {
            if allocation.energies.len() != manager.hand().len()
                || allocation
                    .energies
                    .iter()
                    .any(|value| !(0..=5).contains(value))
            {
                return Err(CardCommandError::InvalidCommand);
            }
            for (card, energy) in manager.hand_mut().iter_mut().zip(allocation.energies) {
                card.energy = Some(energy);
            }
            (
                Some(allocation.origin),
                CardChangeKind::EnergyAllocated,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::ChangeBasicEnergy(change) => {
            if change.delta == 0 || change.count <= 0 {
                return Err(CardCommandError::InvalidCommand);
            }
            manager.add_basic_card_energy(change.delta, change.count);
            (
                Some(change.origin),
                CardChangeKind::EnergyChanged,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::ClearEnergy { origin } => {
            manager.clear_energy();
            (
                Some(origin),
                CardChangeKind::EnergyCleared,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::ChangeHandLimit(change) => {
            if change.delta == 0 || change.resulting_limit < 0 {
                return Err(CardCommandError::InvalidCommand);
            }
            manager.add_hand_limit_bonus(change.delta);
            hand_limit = Some((change.target_uid, change.resulting_limit));
            (
                Some(change.origin),
                CardChangeKind::HandLimitChanged,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::ClearHandLimit { origin } => {
            manager.clear_hand_limit_bonus();
            (
                Some(origin),
                CardChangeKind::HandLimitCleared,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::RankUpQueued {
            origin,
            after_card_index,
            count,
            levels,
        } => {
            if after_card_index <= 0 || count <= 0 || levels <= 0 {
                return Err(CardCommandError::InvalidCommand);
            }
            manager.rank_up_played_after(after_card_index, count, levels);
            (
                Some(origin),
                CardChangeKind::QueuedRankChanged,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::RankUpQueuedCards {
            origin,
            upgrades,
            rewritten,
        } => {
            if upgrades.is_empty()
                || upgrades
                    .iter()
                    .any(|upgrade| upgrade.card_index <= 0 || upgrade.levels <= 0)
            {
                return Err(CardCommandError::InvalidCommand);
            }
            for upgrade in upgrades {
                manager.rank_up_played(upgrade.card_index, upgrade.levels, rewritten);
            }
            (
                Some(origin),
                CardChangeKind::QueuedRankChanged,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::ChangeAroundQueuedRanks { origin, changes } => {
            if changes.is_empty()
                || changes
                    .iter()
                    .any(|change| change.card_index <= 0 || change.levels == 0)
            {
                return Err(CardCommandError::InvalidCommand);
            }
            for change in changes {
                match manager.change_played_rank(change.card_index, change.levels) {
                    Ok(applied) => rank_results.push(CardRankResult::Changed(Box::new(applied))),
                    Err(failure) => rank_results.push(CardRankResult::Failed(failure)),
                }
            }
            (
                Some(origin),
                CardChangeKind::AroundRanksChanged,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::RankUpHand(change) => {
            rank_results.push(CardRankResult::Changed(Box::new(
                manager.rank_up_hand(change.owner_uid, change.hand_index)?,
            )));
            (
                Some(change.origin),
                CardChangeKind::HandRankChanged,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::CommitActionQueue { team, emitter_uid } => {
            let cards = manager
                .played()
                .iter()
                .map(|played| {
                    let mut card = played.card.clone();
                    card.skill_id = Some(played.skill_id);
                    card
                })
                .collect::<Vec<_>>();
            if team == 0 || cards.is_empty() {
                return Err(CardCommandError::InvalidCommand);
            }
            action_queue = Some(CardActionQueue {
                team,
                emitter_uid,
                cards,
                deck_num: manager.deck_num(),
            });
            (
                None,
                CardChangeKind::ActionQueueCommitted,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::ResolvePlayedRanks { origin } => {
            rank_results.extend(
                manager
                    .resolve_played_ranks()
                    .into_iter()
                    .map(|change| CardRankResult::Changed(Box::new(change))),
            );
            (
                Some(origin),
                CardChangeKind::PlayedRanksResolved,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::RecordCastChannel(record) => {
            if record.buff_uid <= 0 {
                return Err(CardCommandError::InvalidCommand);
            }
            manager.record_cast_channel(record.buff_uid, record.cards);
            (
                Some(record.origin),
                CardChangeKind::CastChannelRecorded,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::ResolveCastChannel { origin, buff_uid } => {
            if buff_uid <= 0 || !manager.resolve_cast_channel(buff_uid) {
                return Err(CardCommandError::InvalidCommand);
            }
            (
                Some(origin),
                CardChangeKind::CastChannelResolved,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::RemoveCastChannel { origin, buff_uid } => {
            if buff_uid <= 0 || !manager.remove_cast_channel(buff_uid) {
                return Err(CardCommandError::InvalidCommand);
            }
            (
                Some(origin),
                CardChangeKind::CastChannelRemoved,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
        CardCommand::QueueUseCard(queue) => {
            if queue.card_index <= 0
                || queue.card.skill_id.unwrap_or_default() <= 0
                || queue.team_type == 0
            {
                return Err(CardCommandError::InvalidCommand);
            }
            let queued = QueuedUseCard {
                card_index: queue.card_index,
                card: queue.card,
                team_type: queue.team_type,
                source_skill_id: queue.source_skill_id,
                action: queue.action,
            };
            manager.queue_use_card(queued.clone());
            queued_use_card = Some(queued);
            (
                Some(queue.origin),
                CardChangeKind::UseCardQueued,
                None,
                None,
                Vec::new(),
                Vec::new(),
            )
        }
    };
    let mut after = manager.hand().to_vec();
    if kind == CardChangeKind::ActionQueueCommitted {
        after.extend_from_slice(manager.team_cards());
    }
    Ok(CardChanges {
        origin,
        kind,
        before,
        after,
        added,
        played,
        drawn,
        composed_owners,
        operation,
        action_queue,
        queued_use_card,
        rank_results,
        consumed_indices,
        entity: None,
        hand_limit,
        owner_removal,
        ai_queue,
        team_cards,
    })
}

fn total_energy(cards: &[CardInfo]) -> i32 {
    cards
        .iter()
        .map(|card| card.energy.unwrap_or_default())
        .fold(0_i32, i32::saturating_add)
}

#[cfg(test)]
mod test;
