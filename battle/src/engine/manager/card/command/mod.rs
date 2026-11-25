use sonettobuf::{CardInfo, FightEntityInfo};

use crate::engine::skill::rule::{DefinitionKey, RuleDomain};
use crate::engine::{event::payload::BattleEvent, skill::rule::CommandOrigin};

use super::{
    CardChange, CardManager, CardPlayChoice, CardPrepareAiSkill, EnchantedType, PlayedCard,
    PreparedAiSkill, SovereignEnhancement,
};

mod execute;

pub(super) use execute::execute;

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
pub enum TemporaryCardKind {
    GenericSkill,
    ConfiguredSkill,
    PrecastSkill,
    ConfiguredSkill3,
    RoundStartSkill3,
    HeroSkill,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardAddTemporary {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub skill_id: i32,
    pub hero_id: Option<i32>,
    pub reserve_id: i64,
    pub team_type: i32,
    pub kind: TemporaryCardKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardAddGenerated {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub skill_id: i32,
    pub hero_id: Option<i32>,
    pub team_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardAddHedone {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub skill_id: i32,
    pub hero_id: i32,
    pub team_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardInsertSovereign {
    pub origin: CommandOrigin,
    pub target_uid: i64,
    pub skill_id: i32,
    pub hero_id: i32,
    pub max_position: usize,
    pub team_type: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardInitialSovereignBoost {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub index: usize,
    pub distance: usize,
    pub team_type: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SovereignBoostChange {
    pub from_index: usize,
    pub to_index: usize,
    pub card: CardInfo,
    pub team_type: i32,
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
pub struct CardPersistTemporary {
    pub origin: CommandOrigin,
    pub indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardEnergyAllocation {
    pub origin: CommandOrigin,
    pub energies: Vec<i32>,
    pub limit: i32,
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
    pub normal_deck_draws: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardRefillOne {
    pub origin: CommandOrigin,
    pub card: CardInfo,
    pub stage: CardRefillStage,
    pub consume_draw_pile: bool,
    pub consume_deck: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardRefillStage {
    Opening,
    AfterActions,
    RoundStart,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardSetUltimateAvailability {
    pub origin: CommandOrigin,
    pub card: CardInfo,
    pub available: bool,
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
pub struct CardRecordAiCast {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardRemoveOwner {
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
pub struct CardInvalidateAiOwner {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub team_type: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardInvalidatedAi {
    pub card_index: i32,
    pub team_type: i32,
    pub config_effect: i32,
    pub card: CardInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardInvalidatePlayed {
    pub origin: CommandOrigin,
    pub card_index: i32,
    pub restore: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardDeckRankUpRange {
    pub origin: CommandOrigin,
    pub from: usize,
    pub to: usize,
    pub rank_delta: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardQueueUse {
    pub origin: CommandOrigin,
    pub card_index: i32,
    pub card: CardInfo,
    pub team_type: i32,
    pub source_skill_id: i32,
    pub reserve_id: i32,
    pub action: Option<crate::engine::skill::action::SkillInvocation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct QueuedUseCard {
    pub card_index: i32,
    pub card: CardInfo,
    pub team_type: i32,
    pub source_skill_id: i32,
    pub reserve_id: i32,
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
    pub kind: CardConsumptionKind,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CardConsumptionKind {
    #[default]
    Generic,
    PaperCircle,
    Hedone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardUnlockHedone {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub skill_id: i32,
    pub team_type: i32,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum CardCommand {
    Setup(CardSetup),
    PreserveRefillFloor,
    PreserveDeckCount,
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
    AddHedone(CardAddHedone),
    InsertSovereign(CardInsertSovereign),
    InitialSovereignBoost(CardInitialSovereignBoost),
    ShiftSovereign {
        origin: CommandOrigin,
        owner_uid: i64,
        distance: usize,
        team_type: i32,
    },
    UnlockSovereign {
        origin: CommandOrigin,
        owner_uid: i64,
        team_type: i32,
    },
    UnlockHedone(CardUnlockHedone),
    ChargeSovereign {
        origin: CommandOrigin,
        owner_uid: i64,
        kind: SovereignEnhancement,
        team_type: i32,
    },
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
    PersistTemporary(CardPersistTemporary),
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
    SetUltimateAvailability(CardSetUltimateAvailability),
    RefreshAiQueue(CardRefreshAiQueue),
    PrepareAiSkill(CardPrepareAiSkill),
    SetAiQueue(CardSetAiQueue),
    RecordAiCast(CardRecordAiCast),
    SetTeamCards(CardSetTeamCards),
    RemoveOwner(CardRemoveOwner),
    InvalidateAiOwner(CardInvalidateAiOwner),
    ReplaceOwnerSkills(CardReplaceOwnerSkills),
    ReplaceUpgradeSkills(CardReplaceOwnerSkills),
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
    RewriteQueuedRanks {
        origin: CommandOrigin,
        upgrades: Vec<QueuedCardRankUp>,
    },
    ChangeQueuedRanks {
        origin: CommandOrigin,
        changes: Vec<QueuedCardRankChange>,
    },
    RankUpHand(HandCardRankUp),
    RankUpDeckRange(CardDeckRankUpRange),
    CompleteActionPhase {
        origin: CommandOrigin,
        carried_cards: Vec<(i64, i32)>,
    },
    BeginPostActionRefill {
        origin: CommandOrigin,
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

impl CardCommand {
    pub const fn remove_owner(command: CardRemoveOwner) -> Self {
        Self::RemoveOwner(command)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardChangeKind {
    SovereignCharged,
    SovereignUnlocked,
    HedoneUnlocked,
    InitialSovereignBoost,
    SovereignShifted,
    SovereignInserted,
    Setup,
    ConsumedForEffect,
    Moved,
    Dissolved,
    GeneratedAdded,
    OwnedGeneratedAdded,
    HedoneAdded,
    UniversalAdded,
    RedealtKeepRanks,
    GenericTemporaryAdded,
    TemporaryAdded,
    ConfiguredSkill3Added,
    HeroTemporaryAdded,
    CrystalAdded,
    PrecastAdded,
    TemporaryChanged,
    Enchanted,
    TemporaryPersisted,
    TemporaryExpired,
    Played,
    Drawn,
    OpeningDrawn,
    DrawPileRecycled,
    Refilled,
    UltimateAvailabilityChanged,
    AiQueueRefreshed,
    AiSkillPrepared,
    AiQueueSet,
    AiCastRecorded,
    TeamCardsSet,
    AiOwnerRemoved,
    AiCardsInvalidated,
    OwnerSkillsReplaced,
    UpgradeSkillsReplaced,
    PlayedInvalidated,
    Composed,
    EnergyAllocated,
    EnergyChanged,
    EnergyCleared,
    HandLimitChanged,
    HandLimitCleared,
    QueuedRankChanged,
    QueuedRanksChanged,
    HandRankChanged,
    DeckTopRanksChanged,
    ActionPhaseCompleted,
    PostActionRefillStarted,
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
    pub queued_use_card: Option<QueuedUseCard>,
    pub rank_results: Vec<CardRankResult>,
    pub consumed_indices: Vec<usize>,
    pub expired_indices: Vec<usize>,
    pub consumed_owner_uid: Option<i64>,
    pub consumption_kind: Option<CardConsumptionKind>,
    pub entity: Option<FightEntityInfo>,
    pub hand_limit: Option<(i64, i32)>,
    pub owner_removal: Option<CardOwnerRemoval>,
    pub sovereign_boost: Option<SovereignBoostChange>,
    pub ai_invalidations: Vec<CardInvalidatedAi>,
    pub ai_queue: Option<Vec<CardInfo>>,
    pub team_cards: Option<Vec<CardInfo>>,
}

impl CardChanges {
    pub fn events(&self) -> Vec<BattleEvent> {
        let mut events: Vec<_> = self
            .origin
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
            .collect();
        if let (
            Some(origin),
            Some(CardChange::InsertHand {
                target_uid,
                index,
                team_type,
                ..
            }),
        ) = (self.origin, &self.operation)
        {
            events.push(BattleEvent::SovereignCardCreated(
                crate::engine::event::payload::SovereignCardCreatedEvent {
                    origin,
                    owner_uid: *target_uid,
                    index: *index,
                    team_type: *team_type,
                },
            ));
        }
        events
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardCommandError {
    PlayForbidden,
    InvalidCommand,
    InvalidPlaySkill,
    InvalidPlayIndex,
    PlayedCardNotFound,
}

fn total_energy(cards: &[CardInfo]) -> i32 {
    cards
        .iter()
        .map(|card| card.energy.unwrap_or_default())
        .fold(0_i32, i32::saturating_add)
}

#[cfg(test)]
mod tests;
