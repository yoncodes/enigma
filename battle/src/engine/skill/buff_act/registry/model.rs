use crate::engine::{
    entity::attr::AttrId,
    event::{
        kind::EventKind,
        payload::BattleEvent,
        subscription::{PublicationPhase, ReactionTiming},
    },
    manager::{BattleManagers, buff::ActiveBuffFeature},
    runtime::determinism::RoundDeterminism,
    skill::{
        buff_act,
        effect::SkillEffectCatalog,
        rule::{DefinitionKey, RuleReferences, SetupStage, output::RuleOp},
        subscriber::{BuffActSetupSubscriber, BuffActSubscriber},
        target::TargetPool,
    },
};

use super::{BuffActKind, ParsedBuffAct};

pub struct RuntimeContext<'a> {
    pub managers: &'a BattleManagers,
    pub pool: &'a TargetPool,
    pub catalog: &'a SkillEffectCatalog,
    pub determinism: &'a mut RoundDeterminism,
    pub subscriber: &'a BuffActSubscriber,
    pub event: Option<&'a BattleEvent>,
}

pub type RuntimeHandler = for<'a> fn(&mut RuntimeContext<'a>) -> Option<Vec<RuleOp>>;
pub type ScopedRuntimeHandler =
    for<'a> fn(&mut RuntimeContext<'a>) -> Option<Vec<buff_act::BuffActRuleOp>>;
pub type FallibleScopedRuntimeHandler =
    for<'a> fn(
        &mut RuntimeContext<'a>,
    ) -> Result<Vec<buff_act::BuffActRuleOp>, buff_act::BuffActRuleError>;
pub type TransactionHandler = fn(&BattleManagers, &BattleEvent) -> Vec<(ActiveBuffFeature, RuleOp)>;
type ShieldHitHandler = fn(
    &BattleManagers,
    &ActiveBuffFeature,
    &crate::engine::manager::hp::HpChanges,
    DefinitionKey,
) -> Option<buff_act::shield_counter::Credit>;
#[derive(Debug, Clone, Copy)]
pub(crate) struct ShieldHitRoute {
    pub(crate) effect_time: i32,
    pub(crate) handler: ShieldHitHandler,
    pub(crate) admits_cast: fn(&BattleManagers, i64) -> bool,
}

pub(crate) type ReservePromotionHandler =
    fn(&crate::engine::fight::reserve::Promotion) -> crate::engine::fight::reserve::CycleCommand;

pub struct SetupContext<'a> {
    pub pool: &'a crate::engine::skill::target::TargetPool,
    pub managers: &'a BattleManagers,
    pub catalog: &'a SkillEffectCatalog,
    pub subscriber: &'a BuffActSetupSubscriber,
    pub defender_start_round: Option<i32>,
}

pub type SetupHandler = for<'a> fn(&SetupContext<'a>) -> Option<Vec<RuleOp>>;

pub type SupportsHandler = fn(&[i32]) -> bool;
pub type RawSupportsHandler = fn(Option<&config::GameDB>, &str) -> bool;
pub type FeatureParser = fn(&[String]) -> Option<Vec<i32>>;
pub type FeatureReferences = fn(Option<&config::GameDB>, &ParsedBuffAct) -> RuleReferences;
pub type AttackReplacementHandler = fn(
    &ActiveBuffFeature,
    &crate::engine::manager::hp::HpManager,
) -> Option<buff_act::AttackReplacement>;
pub type HpLossStateHandler =
    fn(&BattleManagers, &ActiveBuffFeature, i32) -> Option<Vec<buff_act::HpLossStateUpdate>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatReadTiming {
    None,
    OnGrant,
    OnTrigger,
    ByArguments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMarkerPosition {
    BeforeChanges,
    AfterFirstChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMarkerTarget {
    Owner,
    Source,
    EventSource,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimeFrameScope {
    CausingFrame,
    #[default]
    SubscriberFrame,
    IndependentEvent,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimeFrameSource {
    #[default]
    Counterparty,
    Owner,
    Applier,
    EventTarget,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SetupFrameScope {
    #[default]
    SubscriberFrame,
    SideSubscriberFrame,
    MechanicFrame,
    RootMechanicFrame,
    IndependentStep,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimeTeamScope {
    #[default]
    Same,
    Opposing,
    Any,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimeActorScope {
    #[default]
    Owner,
    Team,
    OpposingTeam,
    // Actions taken by the owner and actions that target the owner.
    OwnerOrTarget,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimeEventMultiplicity {
    #[default]
    EveryEvent,
    OncePerActionTarget,
    OncePerBatchTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeMarker {
    pub position: RuntimeMarkerPosition,
    pub target: RuntimeMarkerTarget,
    pub effect_type: Option<i32>,
}

type RuntimeTiming = fn(crate::catalog::BattleCatalog, i32, &[i32]) -> Option<ReactionTiming>;

#[derive(Debug, Clone, Copy)]
pub struct BuffActRuntimeDefinition {
    pub effect_time_subscription: bool,
    pub event_override: Option<EventKind>,
    pub phase_override: Option<crate::engine::skill::action::SkillPhase>,
    pub events: &'static [EventKind],
    pub publication: PublicationPhase,
    pub publications: &'static [(EventKind, PublicationPhase)],
    pub frame_scope: RuntimeFrameScope,
    pub frame_source: RuntimeFrameSource,
    pub team_scope: RuntimeTeamScope,
    pub actor_scope: RuntimeActorScope,
    pub reaction_timing: ReactionTiming,
    pub(super) timing_override: Option<RuntimeTiming>,
    pub event_multiplicity: RuntimeEventMultiplicity,
    pub reserves_trigger_child_uid: bool,
    pub owns_duration: bool,
    pub(crate) non_attack_duration: Option<(i32, crate::engine::skill::action::SkillPhase)>,
    pub marker: Option<RuntimeMarker>,
    pub handler: Option<RuntimeHandler>,
    pub scoped_handler: Option<ScopedRuntimeHandler>,
    pub fallible_scoped_handler: Option<FallibleScopedRuntimeHandler>,
}

impl BuffActRuntimeDefinition {
    pub(crate) fn timing_for(
        self,
        catalog: crate::catalog::BattleCatalog,
        buff_id: i32,
        args: &[i32],
    ) -> ReactionTiming {
        self.timing_override
            .and_then(|timing| timing(catalog, buff_id, args))
            .unwrap_or(self.reaction_timing)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BuffActSetupDefinition {
    pub routes: &'static [(SetupStage, i32)],
    pub side_subscriber_steps: &'static [(SetupStage, i32)],
    pub mechanic_steps: &'static [(SetupStage, i32)],
    pub root_mechanic_steps: &'static [(SetupStage, i32)],
    pub independent_steps: &'static [(SetupStage, i32, i32)],
    pub handler: Option<SetupHandler>,
}

#[derive(Debug, Clone, Copy)]
pub struct BuffActTransactionDefinition {
    pub events: &'static [EventKind],
    pub wire_marker_events: &'static [EventKind],
    pub handler: Option<TransactionHandler>,
    pub replacement_publication: Option<PublicationPhase>,
    pub expiry_publications: &'static [(i32, AttrId, PublicationPhase)],
}

#[derive(Debug, Clone, Copy)]
pub struct BuffActStateDefinition {
    pub read_timing: StatReadTiming,
    pub attack_replacement: Option<AttackReplacementHandler>,
    pub hp_loss: Option<HpLossStateHandler>,
    pub(crate) shield_hit: Option<ShieldHitRoute>,
    pub(crate) riposte_damage: Option<(i32, i32)>,
    pub(crate) reserve_promotion: Option<ReservePromotionHandler>,
    pub consumer: bool,
    pub initial: Option<BuffActInitialStateRule>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffActInitialStateRule {
    CrystalSelection,
    ConduitCardSelection,
    ButterflyAllowedSkillKinds,
    ZeroInteger,
    HeatScale,
    CurrentHpPermille,
    SourceAttackThreshold,
    SourceAttackAttribute,
    SourceAttack,
    FoundationCounter,
    FirstArgument,
    SecondArgument,
    StringCounter,
    GrantValue,
    LostHpToFakeHp,
    HedoneEnergy,
}

#[derive(Debug, Clone, Copy)]
pub struct BuffActDefinition {
    pub key: DefinitionKey,
    pub kind: BuffActKind,
    pub runtime: BuffActRuntimeDefinition,
    pub setup: BuffActSetupDefinition,
    pub transaction: BuffActTransactionDefinition,
    pub state: BuffActStateDefinition,
    pub supports: Option<SupportsHandler>,
    pub raw_supports: Option<RawSupportsHandler>,
    pub parser: Option<FeatureParser>,
    pub references: FeatureReferences,
    pub description_required: bool,
    pub completion_gap: Option<&'static str>,
    pub wire: Option<buff_act::wire::BuffActWireDefinition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffActDestination {
    Runtime,
    Transaction,
    Setup,
    AttackReplacement,
    HpLossState,
    StateConsumer,
    LinkedSkill,
}

impl BuffActDefinition {
    pub(crate) fn accepts_route(&self, effect_time: i32, effect_condition: i32) -> bool {
        self.state
            .shield_hit
            .is_none_or(|route| route.effect_time == effect_time)
            && self
                .state
                .riposte_damage
                .is_none_or(|route| route == (effect_time, effect_condition))
    }

    pub fn validate_arguments(
        &self,
        game: Option<&config::GameDB>,
        args: &[i32],
        raw: Option<&str>,
    ) -> bool {
        if let Some(supports) = self.raw_supports {
            return raw.is_some_and(|raw| supports(game, raw));
        }
        self.supports.is_none_or(|supports| supports(args))
    }

    pub fn setup_frame(&self, stage: SetupStage, priority: i32) -> (SetupFrameScope, i32) {
        if self
            .setup
            .side_subscriber_steps
            .contains(&(stage, priority))
        {
            return (SetupFrameScope::SideSubscriberFrame, 0);
        }
        if self.setup.root_mechanic_steps.contains(&(stage, priority)) {
            return (SetupFrameScope::RootMechanicFrame, 0);
        }
        if self.setup.mechanic_steps.contains(&(stage, priority)) {
            return (SetupFrameScope::MechanicFrame, 0);
        }
        self.setup
            .independent_steps
            .iter()
            .find(|(route_stage, route_priority, _)| {
                *route_stage == stage && *route_priority == priority
            })
            .map(|(_, _, order)| (SetupFrameScope::IndependentStep, *order))
            .unwrap_or_default()
    }

    pub(super) fn destination(&self) -> Option<BuffActDestination> {
        if self.runtime.handler.is_some()
            || self.runtime.scoped_handler.is_some()
            || self.runtime.fallible_scoped_handler.is_some()
        {
            Some(BuffActDestination::Runtime)
        } else if self.transaction.handler.is_some() {
            Some(BuffActDestination::Transaction)
        } else if self.setup.handler.is_some() {
            Some(BuffActDestination::Setup)
        } else if self.state.attack_replacement.is_some() {
            Some(BuffActDestination::AttackReplacement)
        } else if self.state.hp_loss.is_some() {
            Some(BuffActDestination::HpLossState)
        } else if self.state.consumer
            || self.state.reserve_promotion.is_some()
            || self.state.shield_hit.is_some()
        {
            Some(BuffActDestination::StateConsumer)
        } else {
            None
        }
    }
}
