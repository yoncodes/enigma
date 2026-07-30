use crate::engine::{
    event::{kind::EventKind, subscription::PublicationPhase},
    skill::{
        effect::SkillEffectCatalog,
        rule::{DefinitionKey, SetupStage, output::RuleOp},
        subscriber::{BuffActSetupSubscriber, BuffActSubscriber},
        target::TargetPool,
    },
};
use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::payload::BattleEvent,
    manager::{BattleManagers, buff::ActiveBuffFeature},
    runtime::determinism::RoundDeterminism,
};

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
    for<'a> fn(&mut RuntimeContext<'a>) -> Option<Vec<super::BuffActRuleOp>>;
pub type TransactionHandler = fn(&BattleManagers, &BattleEvent) -> Vec<(ActiveBuffFeature, RuleOp)>;

pub struct SetupContext<'a> {
    pub managers: &'a BattleManagers,
    pub catalog: &'a SkillEffectCatalog,
    pub subscriber: &'a BuffActSetupSubscriber,
}

pub type SetupHandler = for<'a> fn(&SetupContext<'a>) -> Option<Vec<RuleOp>>;

pub type SupportsHandler = fn(&[i32]) -> bool;
pub type AttackReplacementHandler = fn(
    &ActiveBuffFeature,
    &crate::engine::manager::hp::HpManager,
) -> Option<super::AttackReplacement>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffActKind {
    AddAttrByOtherBuffLayer,
    AddAttrBySpecialCount,
    AddAssassinateY,
    AddBuffByChargingTimes,
    AddBuffBoth,
    AddBuffByOtherExSkill,
    AddBuffToEnter,
    AddCardCastChannel,
    AddCardRecordByRound,
    AdrenalineAddCard,
    AddPassiveSkills,
    AddSplitEmitterNum,
    AddSpTempCard,
    AddToBuffEntity2,
    AddBuffAfterAttack,
    AddToAttackTargets,
    AddToTarget,
    AttackNumSplitEmitterNum,
    Attr,
    AttrAndLayerAttr,
    AttrByDamageType,
    AttrByHeroId,
    AttrByLostHp,
    AttrByShield,
    AttrByHeatScale,
    AttrFromEntity,
    AttrOnlyCalDamageAttack,
    AttrOnlyCalDamageAttackBigSkill,
    AttrOnlyCalDamageBeAttacked,
    AttrOnlyCalDamageInExtra,
    AttrOnlyCalDamageHpReplaceAttackCalSkillDamage,
    AttrOnlyCalDamageReplaceAttr,
    AttrOnlyCalDamageReplaceAttrAdCreator,
    BeAttackByEmitterDamage,
    BeAttackedAssassinate,
    BeatBack,
    BeatBackDependOnAttackMe,
    BuffAddAct,
    BuffAddActLimit,
    BuffReplace,
    BloodPoolCountAddExPoint,
    BloodPoolTag,
    BloodValueUseSkill,
    ButterflyRecordSkill,
    BigSkillNoUseActPoint,
    BanLostLife,
    Bullet,
    Burn,
    CardLimitAdd,
    CardNotCalSize,
    EntityExSkillNotCalSize,
    CareerRatioFix,
    CareerRestraint,
    CastChannel,
    ConsumeBuffAddBuffContinueChannel,
    ConsumeBuffContinueChannel,
    ControlTeamInjuryCountRound,
    ConduitCardSelection,
    CreateAdditionalDamage,
    CreateMaxHpAdditionalDamageAndRemove,
    CritRateAlter2,
    CritRateAlterByOtherBuff,
    CrystalNotifySelect,
    CrystalAddBuff,
    Cure,
    AdvancedCure,
    CureUpByLostHp,
    DamageNotMoreThan,
    DeviceCostReduce,
    Dot,
    DotNoLimit,
    DodgeDamageType,
    DodgeSpecSkill,
    DuduBoneContinueChannel,
    DyingHealDisperse1,
    DeadlyPoison,
    Disarm,
    Dizzy,
    EmitterCardAllocateChange,
    EmitterCareerChange,
    EmitterDamageUp,
    EmitterEnergyAddBuff,
    EmitterFixSubTargetsDamageReduceRate,
    EmitterNumChange,
    EmitterRendTarget,
    EmitterTag,
    ExtraValueElectricTransform,
    EzioBigSkill,
    EachChangeAttr,
    ExPointAddByHit,
    ExPointCardMove,
    ExPointCantAdd,
    ExSkillPointChange,
    ExPointMaxAdd,
    ExPointOverflowBank,
    FixAttrBySubBuffLayer,
    FixAttrByTeammateInjuryCountNotReset,
    FixAttrTeamEnergy,
    FixAttrTeamEnergyAndBuff,
    FixElectricUpgrade,
    FixedHurt,
    FixTempAttrByBuffLayer,
    MustCritAndFixTempAttr,
    Forbid,
    Seal,
    Sleep,
    CantGetExskill,
    HeatScaleAddFix,
    HeatScaleBurnAddFix,
    HeatScaleDecrCounter,
    HeatScaleTag,
    HeatScaleUseSkill,
    InjuryBank,
    InjuryLogback,
    ImmunityTimes,
    AttrFixFromInjuryBank,
    AbsorbHurt,
    LayerMasterHalo,
    LostHpAddExtraBloodPoolValue,
    LostHpCountAddBuff,
    LifeAttackFixRate,
    MonitorContinueChannel,
    ModifyMaxBuffLayers,
    ModifyMaxBurnLayers,
    MasterHalo,
    MockTaunt,
    MonsterLabel,
    NuoDiKaCastChannel,
    PowerMaxAdd,
    PaperCircleContinueChannel,
    Poison,
    PoisonSettleCanCrit,
    Provoke,
    RaspberryBigSkill,
    Raspberry,
    Radiance,
    RealHarmFix,
    RealHarmSkillEffectFix,
    RealHurtFix,
    RealDamageKill,
    Rebound,
    Revive,
    Shield,
    ShareHurt,
    SlaveHalo,
    Shell,
    ShellDebuff,
    ShellLock,
    ShellProcess,
    SpecialCountContinueChannelBuff,
    SpecialCountCastChannel,
    SubBuff,
    Taunt,
    TeamImmunityTimes,
    TeamExElectricTransConsumeValueAttr,
    TeamShareShield,
    RecordTeamExElectricTransConsumeValue,
    TargetingTag,
    TeammateInjuryCount,
    TransferEnergyBuff,
    UseSkillTeamAddEmitterEnergy,
    UseSkillAttrFix,
    UseSkillLoseHpNotFixed,
    UseCardFixExPoint,
    UseDamageSkillAddToTarget,
    UseSkillToEnemy,
}

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
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimeSettlementPhase {
    #[default]
    Before,
    After,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimeExecutionTiming {
    #[default]
    Immediate,
    AfterAction,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RuntimeEventMultiplicity {
    #[default]
    EveryEvent,
    OncePerActionTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeMarker {
    pub position: RuntimeMarkerPosition,
    pub target: RuntimeMarkerTarget,
}

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
    pub settlement_phase: RuntimeSettlementPhase,
    pub execution_timing: RuntimeExecutionTiming,
    pub event_multiplicity: RuntimeEventMultiplicity,
    pub reserves_trigger_child_uid: bool,
    pub marker: Option<RuntimeMarker>,
    pub handler: Option<RuntimeHandler>,
    pub scoped_handler: Option<ScopedRuntimeHandler>,
}

#[derive(Debug, Clone, Copy)]
pub struct BuffActSetupDefinition {
    pub routes: &'static [(SetupStage, i32)],
    pub mechanic_steps: &'static [(SetupStage, i32)],
    pub root_mechanic_steps: &'static [(SetupStage, i32)],
    pub independent_steps: &'static [(SetupStage, i32, i32)],
    pub handler: Option<SetupHandler>,
}

#[derive(Debug, Clone, Copy)]
pub struct BuffActTransactionDefinition {
    pub events: &'static [EventKind],
    pub handler: Option<TransactionHandler>,
}

#[derive(Debug, Clone, Copy)]
pub struct BuffActStateDefinition {
    pub read_timing: StatReadTiming,
    pub attack_replacement: Option<AttackReplacementHandler>,
    pub consumer: bool,
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
    pub wire: Option<super::wire::BuffActWireDefinition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffActDestination {
    Runtime,
    Transaction,
    Setup,
    AttackReplacement,
    StateConsumer,
    LinkedSkill,
}

impl BuffActDefinition {
    pub fn setup_frame(&self, stage: SetupStage, priority: i32) -> (SetupFrameScope, i32) {
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

    fn destination(&self) -> Option<BuffActDestination> {
        if self.runtime.handler.is_some() || self.runtime.scoped_handler.is_some() {
            Some(BuffActDestination::Runtime)
        } else if self.transaction.handler.is_some() {
            Some(BuffActDestination::Transaction)
        } else if self.setup.handler.is_some() {
            Some(BuffActDestination::Setup)
        } else if self.state.attack_replacement.is_some() {
            Some(BuffActDestination::AttackReplacement)
        } else if self.state.consumer {
            Some(BuffActDestination::StateConsumer)
        } else {
            None
        }
    }
}

macro_rules! buff_act_definitions {
    (
        $(
            ($id:expr, $type_name:literal) => $kind:ident
            $(, effect_time_subscription: $effect_time_subscription:expr)?
            $(, event: $event:expr)?
            $(, phase: $phase:ident)?
            $(, events: [$($events:expr),*])?
            $(, transactions: [$($transaction_events:expr),*])?
            $(, setup: [$($stage:ident($priority:expr)),*])?
            $(, mechanic_setup: [$($mechanic_stage:ident($mechanic_priority:expr)),*])?
            $(, root_mechanic_setup: [$($root_mechanic_stage:ident($root_mechanic_priority:expr)),*])?
            $(, independent_setup: [$($independent_stage:ident($independent_priority:expr, $setup_order:expr)),*])?
            $(, publication: $publication:ident)?
            $(, publications: [$($publication_event:expr => $event_phase:ident),*])?
            $(, frame: $frame:ident)?
            $(, source: $source:ident)?
            $(, team: $team:ident)?
            $(, actor: $actor:ident)?
            $(, settlement: $settlement:ident)?
            $(, timing: $timing:ident)?
            $(, multiplicity: $multiplicity:ident)?
            $(, trigger_child_uid: $trigger_child_uid:expr)?
            $(, stat_read: $stat_read:ident)?
            $(, runtime_marker: $marker_position:ident($marker_target:ident))?
            $(, runtime: $runtime:expr)?
            $(, scoped_runtime: $scoped_runtime:expr)?
            $(, transaction: $transaction:expr)?
            $(, setup_handler: $setup_handler:expr)?
            $(, supports: $supports:expr)?
            $(, attack_replacement: $attack_replacement:expr)?
            $(, state_consumer: $state_consumer:expr)?
            $(, wire: ($wire:expr))?
        );*
        $(;)?
    ) => {
        pub const DEFINITIONS: &[BuffActDefinition] = &[
            $(BuffActDefinition {
                key: DefinitionKey::new($id, $type_name),
                kind: BuffActKind::$kind,
                runtime: BuffActRuntimeDefinition {
                    effect_time_subscription: buff_act_definitions!(@effect_time_subscription $($effect_time_subscription)?),
                    event_override: buff_act_definitions!(@event $($event)?),
                    phase_override: buff_act_definitions!(@phase $($phase)?),
                    events: &[$($($events),*)?],
                    publication: buff_act_definitions!(@publication $($publication)?),
                    publications: &[$($(($publication_event, PublicationPhase::$event_phase)),*)?],
                    frame_scope: buff_act_definitions!(@frame $($frame)?),
                    frame_source: buff_act_definitions!(@source $($source)?),
                    team_scope: buff_act_definitions!(@team $($team)?),
                    actor_scope: buff_act_definitions!(@actor $($actor)?),
                    settlement_phase: buff_act_definitions!(@settlement $($settlement)?),
                    execution_timing: buff_act_definitions!(@timing $($timing)?),
                    event_multiplicity: buff_act_definitions!(@multiplicity $($multiplicity)?),
                    reserves_trigger_child_uid: buff_act_definitions!(@trigger_child_uid $($trigger_child_uid)?),
                    marker: buff_act_definitions!(@runtime_marker $($marker_position($marker_target))?),
                    handler: buff_act_definitions!(@runtime $($runtime)?),
                    scoped_handler: buff_act_definitions!(@scoped_runtime $($scoped_runtime)?),
                },
                setup: BuffActSetupDefinition {
                    routes: buff_act_definitions!(@setup [$($($stage($priority)),*)?]),
                    mechanic_steps: &[$($((SetupStage::$mechanic_stage, $mechanic_priority)),*)?],
                    root_mechanic_steps: &[$($((SetupStage::$root_mechanic_stage, $root_mechanic_priority)),*)?],
                    independent_steps: &[$($((SetupStage::$independent_stage, $independent_priority, $setup_order)),*)?],
                    handler: buff_act_definitions!(@setup_handler $($setup_handler)?),
                },
                transaction: BuffActTransactionDefinition {
                    events: &[$($($transaction_events),*)?],
                    handler: buff_act_definitions!(@transaction $($transaction)?),
                },
                state: BuffActStateDefinition {
                    read_timing: buff_act_definitions!(@stat_read $($stat_read)?),
                    attack_replacement: buff_act_definitions!(@attack_replacement $($attack_replacement)?),
                    consumer: buff_act_definitions!(@state_consumer $($state_consumer)?),
                },
                supports: buff_act_definitions!(@supports $($supports)?),
                wire: buff_act_definitions!(@wire $($wire)?),
            }),*
        ];
    };
    (@effect_time_subscription $value:expr) => { $value };
    (@effect_time_subscription) => { true };
    (@event $event:expr) => { Some($event) };
    (@event) => { None };
    (@phase $phase:ident) => { Some(crate::engine::skill::action::SkillPhase::$phase) };
    (@phase) => { None };
    (@setup [$($stage:ident($priority:expr)),*]) => {
        &[$((SetupStage::$stage, $priority)),*]
    };
    (@publication $publication:ident) => { PublicationPhase::$publication };
    (@publication) => { PublicationPhase::AfterPublish };
    (@frame $frame:ident) => { RuntimeFrameScope::$frame };
    (@frame) => { RuntimeFrameScope::SubscriberFrame };
    (@source $source:ident) => { RuntimeFrameSource::$source };
    (@source) => { RuntimeFrameSource::Counterparty };
    (@team $team:ident) => { RuntimeTeamScope::$team };
    (@team) => { RuntimeTeamScope::Same };
    (@actor $actor:ident) => { RuntimeActorScope::$actor };
    (@actor) => { RuntimeActorScope::Owner };
    (@settlement $settlement:ident) => { RuntimeSettlementPhase::$settlement };
    (@settlement) => { RuntimeSettlementPhase::Before };
    (@timing $timing:ident) => { RuntimeExecutionTiming::$timing };
    (@timing) => { RuntimeExecutionTiming::Immediate };
    (@multiplicity $multiplicity:ident) => { RuntimeEventMultiplicity::$multiplicity };
    (@multiplicity) => { RuntimeEventMultiplicity::EveryEvent };
    (@trigger_child_uid $value:expr) => { $value };
    (@trigger_child_uid) => { false };
    (@stat_read $value:ident) => { StatReadTiming::$value };
    (@stat_read) => { StatReadTiming::None };
    (@runtime_marker $position:ident($target:ident)) => {
        Some(RuntimeMarker {
            position: RuntimeMarkerPosition::$position,
            target: RuntimeMarkerTarget::$target,
        })
    };
    (@runtime_marker) => { None };
    (@runtime $handler:expr) => { Some($handler) };
    (@runtime) => { None };
    (@scoped_runtime $handler:expr) => { Some($handler) };
    (@scoped_runtime) => { None };
    (@transaction $handler:expr) => { Some($handler) };
    (@transaction) => { None };
    (@setup_handler $handler:expr) => { Some($handler) };
    (@setup_handler) => { None };
    (@supports $handler:expr) => { Some($handler) };
    (@supports) => { None };
    (@attack_replacement $handler:expr) => { Some($handler) };
    (@attack_replacement) => { None };
    (@state_consumer $value:expr) => { $value };
    (@state_consumer) => { false };
    (@wire $wire:expr) => { Some($wire) };
    (@wire) => { None };
}

buff_act_definitions! {
    (100, "Attr") => Attr,
        transactions: [EventKind::BuffAdded, EventKind::BuffChanged, EventKind::BuffRemoved],
        publication: BeforePublish, frame: CausingFrame,
        transaction: super::attr::transaction_rule_ops, wire: (super::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(100, "Attr"), &[EffectType::Attr as i32]).with_max_hp(2, 0));
    (853, "AttrByLostHp") => AttrByLostHp, effect_time_subscription: false,
        supports: |args| matches!(args, [step, attrs @ .., max_steps]
            if *step > 0 && !attrs.is_empty() && *max_steps > 0), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(853, "AttrByLostHp"), &[EffectType::Attr as i32]));
    (201, "Cure") => Cure,
        runtime: |context| super::cure::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |args| super::cure::supports(BuffActKind::Cure, args), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(201, "Cure"), &[EffectType::Cure as i32]));
    (203, "Dot") => Dot, stat_read: ByArguments,
        runtime: |context| Some(super::damage_over_time::damage_rule_ops(context.managers, context.pool, context.determinism, context.subscriber)),
        supports: super::damage_over_time::supports_dot, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(203, "Dot"), &[EffectType::Dot as i32]));
    (512, "Cure") => Cure,
        runtime: |context| super::revive::rule_ops(context.managers, context.subscriber, context.event?),
        supports: super::revive::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(512, "Cure"), &[EffectType::Cure as i32]));
    (849, "AdvancedCure") => AdvancedCure, events: [EventKind::BeAttacked],
        runtime_marker: BeforeChanges(Owner),
        runtime: |context| super::cure::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |args| super::cure::supports(BuffActKind::AdvancedCure, args), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(849, "AdvancedCure"), &[EffectType::None as i32]));
    (113, "AttrOnlyCalDamageAttack") => AttrOnlyCalDamageAttack,
        supports: |args| matches!(args, [_, _, consume, ..] if *consume != 0), state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(113, "AttrOnlyCalDamageAttack"), &[EffectType::None as i32]));
    (1001, "AttrOnlyCalDamageAttackBigSkill") => AttrOnlyCalDamageAttackBigSkill,
        effect_time_subscription: false,
        supports: |args| matches!(args, [_, _, consume, ..] if *consume != 0), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1001, "AttrOnlyCalDamageAttackBigSkill"), &[EffectType::Attr as i32]));
    (112, "AttrOnlyCalDamageBeAttacked") => AttrOnlyCalDamageBeAttacked,
        effect_time_subscription: false,
        supports: |args| matches!(args, [_, _, consume, ..] if *consume != 0), state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(112, "AttrOnlyCalDamageBeAttacked"), &[EffectType::None as i32]));
    (740, "AttrOnlyCalDamageInExtra") => AttrOnlyCalDamageInExtra,
        effect_time_subscription: false,
        supports: super::attr_only_cal_damage_attack::supports_extra_action, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(740, "AttrOnlyCalDamageInExtra"), &[EffectType::None as i32]));
    (302, "BeatBack") => BeatBack,
        event: EventKind::SkillAction, phase: HitPassives, frame: CausingFrame, actor: OpposingTeam,
        runtime: |context| super::riposte::holder_rule_ops(context.pool, context.subscriber, context.event?),
        supports: super::riposte::supports_holder, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(302, "BeatBack"), &[EffectType::Beatback as i32]));
    (301, "Taunt") => Taunt, effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(301, "Taunt"), &[EffectType::Taunt as i32]));
    (303, "Rebound") => Rebound, source: Owner,
        multiplicity: OncePerActionTarget,
        runtime_marker: BeforeChanges(EventSource),
        runtime: |context| super::rebound::rule_ops(context.managers, context.subscriber, context.event?),
        supports: super::rebound::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(303, "Rebound"), &[EffectType::Rebound as i32]));
    (401, "Dizzy") => Dizzy, effect_time_subscription: false,
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(401, "Dizzy"), &[EffectType::Dizzy as i32]));
    (403, "Sleep") => Sleep, event: EventKind::TargetAttacked, frame: CausingFrame,
        runtime: |context| super::sleep::rule_ops(context.subscriber, context.event?),
        supports: |args| args.is_empty(), wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(403, "Sleep"), &[EffectType::Sleep as i32]));
    (501, "Shield") => Shield, effect_time_subscription: false,
        supports: super::shield::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(501, "Shield"), &[EffectType::Shield as i32]));
    (511, "FixedHurt") => FixedHurt, effect_time_subscription: false,
        supports: super::fixed_hurt::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(511, "FixedHurt"), &[EffectType::Fixedhurt as i32]));
    (503, "AddToTarget") => AddToTarget,
        publications: [
            EventKind::SkillCast => BeforePublish
        ],
        scoped_runtime: |context| super::add_to_target::scoped_rule_ops(context.subscriber, context.event?, context.catalog, context.pool),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(503, "AddToTarget"), &[EffectType::Addtotarget as i32]));
    (505, "DodgeSpecSkill") => DodgeSpecSkill, effect_time_subscription: false,
        events: [EventKind::AllyAction],
        runtime: |context| super::dodge_spec_skill::expire_after_owner_action(context.subscriber, context.event?),
        supports: super::dodge_spec_skill::supports_skill_slots, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(505, "DodgeSpecSkill"), &[EffectType::Dodgespecskill as i32]));
    (507, "DodgeSpecSkill2") => DodgeDamageType, effect_time_subscription: false,
        events: [EventKind::AllyAction],
        runtime: |context| super::dodge_spec_skill::expire_after_owner_action(context.subscriber, context.event?),
        supports: super::dodge_spec_skill::supports_damage_types, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(507, "DodgeSpecSkill2"), &[EffectType::Dodgespecskill2 as i32]));
    (510, "DamageNotMoreThan") => DamageNotMoreThan, effect_time_subscription: false,
        events: [EventKind::TargetAttacked],
        runtime: |context| super::damage_not_more_than::consume_after_hit(context.subscriber, context.event?),
        supports: super::damage_not_more_than::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(510, "DamageNotMoreThan"), &[EffectType::Damagenotmorethan as i32]));
    (518, "AddToTarget") => AddToTarget,
        scoped_runtime: |context| super::add_to_target::scoped_rule_ops(context.subscriber, context.event?, context.catalog, context.pool),
        supports: |_| true;
    (519, "RealHurtFix") => RealHurtFix, effect_time_subscription: false,
        supports: |args| matches!(args, [value] if *value != 0), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(519, "RealHurtFix"), &[EffectType::Realhurtfix as i32]));
    (520, "RealHarmFix") => RealHarmFix, effect_time_subscription: false,
        supports: |args| matches!(args, [value] if *value != 0), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(520, "RealHarmFix"), &[EffectType::Realharmfix as i32]));
    (522, "RealHarmSkillEffectFix") => RealHarmSkillEffectFix, effect_time_subscription: false, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(522, "RealHarmSkillEffectFix"), &[EffectType::Realharmskilleffectfix as i32]));
    (703, "ExPointMaxAdd") => ExPointMaxAdd,
        transactions: [EventKind::BuffAdded, EventKind::BuffChanged, EventKind::BuffRemoved],
        frame: CausingFrame,
        transaction: super::ex_point_max_transaction_rule_ops, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(703, "ExPointMaxAdd"), &[]));
    (607, "ExPointCardMove") => ExPointCardMove,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(607, "ExPointCardMove"), &[EffectType::Expointcardmove as i32]));
    (603, "ExPointCantAdd") => ExPointCantAdd,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(603, "ExPointCantAdd"), &[EffectType::Expointcantadd as i32]));
    (722, "CantGetExskill") => CantGetExskill,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(722, "CantGetExskill"), &[EffectType::Cantgetexskill as i32]));
    (709, "BuffAddAct") => BuffAddAct, effect_time_subscription: false,
        supports: super::add_action_point::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(709, "BuffAddAct"), &[EffectType::Buffaddact as i32]));
    (719, "PowerMaxAdd") => PowerMaxAdd, effect_time_subscription: false,
        transactions: [EventKind::BuffAdded, EventKind::BuffChanged, EventKind::BuffRemoved],
        frame: CausingFrame,
        transaction: super::power_max_transaction_rule_ops,
        supports: |args| matches!(args, [power_id, delta] if *power_id > 0 && *delta != 0), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(719, "PowerMaxAdd"), &[]));
    (720, "MonsterLabel") => MonsterLabel,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(720, "MonsterLabel"), &[EffectType::Monsterlabelbuff as i32]));
    (721, "DotNoLimit") => DotNoLimit, runtime_marker: BeforeChanges(Owner),
        scoped_runtime: |context| super::dot_no_limit::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(721, "DotNoLimit"), &[EffectType::Dot as i32]));
    (795, "None") => TargetingTag,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(795, "None"), &[EffectType::None as i32]));
    (725, "AddToTarget") => AddToTarget,
        runtime: |context| super::add_to_target::rule_ops(context.subscriber, context.event?, context.catalog, context.pool),
        supports: |_| true;
    (731, "CastChannel") => CastChannel,
        event: EventKind::RoundStart,
        runtime: |context| super::cast_channel::rule_ops(context.subscriber, context.event?),
        supports: super::cast_channel::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(731, "CastChannel"), &[]));
    (726, "Burn") => Burn, stat_read: OnTrigger,
        runtime: |context| Some(super::damage_over_time::damage_rule_ops(context.managers, context.pool, context.determinism, context.subscriber)),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(726, "Burn"), &[EffectType::Burn as i32]));
    (748, "UseDamageSkillAddToTarget") => UseDamageSkillAddToTarget,
        events: [EventKind::SkillCast],
        publications: [
            EventKind::SkillAction => BeforePublish
        ],
        scoped_runtime: |context| super::use_damage_skill_add_to_target::rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(748, "UseDamageSkillAddToTarget"), &[EffectType::None as i32]));
    (757, "UseSkillLoseHpNotFixed") => UseSkillLoseHpNotFixed,
        runtime: |context| super::use_skill_modifier::loss_rule_ops(context.managers, context.subscriber, context.event?),
        supports: super::use_skill_modifier::supports_loss, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(757, "UseSkillLoseHpNotFixed"), &[EffectType::None as i32]));
    (758, "UseSkillAttrFix") => UseSkillAttrFix, effect_time_subscription: false,
        supports: super::use_skill_modifier::supports_attribute, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(758, "UseSkillAttrFix"), &[EffectType::None as i32]));
    (800, "TeammateInjuryCount") => TeammateInjuryCount,
        actor: Team,
        runtime: |context| super::fix_attr_by_teammate_injury_count::tracker_rule_ops(
            context.subscriber,
            context.event?,
        ),
        supports: |args| args.is_empty(), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(800, "TeammateInjuryCount"), &[EffectType::Teammateinjurycount as i32]));
    (801, "FixAttrByTeammateInjuryCountNotReset") => FixAttrByTeammateInjuryCountNotReset,
        effect_time_subscription: false, stat_read: OnTrigger,
        supports: |args| matches!(args, [attr_id, amount, maximum]
            if crate::engine::entity::attr::AttrId::from_raw(*attr_id).is_some()
                && *amount != 0 && *maximum > 0), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(801, "FixAttrByTeammateInjuryCountNotReset"), &[EffectType::None as i32]));
    (803, "Poison") => Poison, stat_read: OnGrant,
        runtime: |context| Some(super::damage_over_time::damage_rule_ops(context.managers, context.pool, context.determinism, context.subscriber)),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(803, "Poison"), &[EffectType::Poison as i32]));
    (812, "PoisonSettleCanCrit") => PoisonSettleCanCrit,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(812, "PoisonSettleCanCrit"), &[EffectType::Poisonsettlecancrit as i32]));
    (846, "DuduBoneContinueChannel") => DuduBoneContinueChannel,
        runtime_marker: BeforeChanges(Owner),
        runtime: |context| super::dudu_bone_continue_channel::rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: |args| matches!(args, [buff_id, compound_cap, first_stacks, ending_skill, target_code]
            if *buff_id > 0 && *compound_cap >= 0 && *first_stacks > 0
                && *ending_skill > 0 && *target_code != 0), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(846, "DuduBoneContinueChannel"), &[EffectType::Dudubonecontinuechannel as i32]));
    (862, "PaperCircleContinueChannel") => PaperCircleContinueChannel,
        runtime: |context| super::paper_circle_continue_channel::rule_ops(context.subscriber, context.event?),
        supports: |args| matches!(args, [skill_id, _, _, pairs @ ..]
            if *skill_id > 0 && pairs.len() >= 2 && pairs.len() % 2 == 0), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(862, "PaperCircleContinueChannel"), &[EffectType::None as i32]));
    (850, "AddBuffBoth") => AddBuffBoth,
        runtime: |context| super::add_buff_both::rule_ops(context.managers, context.pool, context.determinism, context.subscriber, context.event?),
        supports: |args| matches!(args, [enemy_buff_id, ally_target, ally_buff_id]
            if *enemy_buff_id > 0 && *ally_target > 0 && *ally_buff_id > 0), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(850, "AddBuffBoth"), &[EffectType::None as i32]));
    (845, "UseCardFixExPoint") => UseCardFixExPoint,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(845, "UseCardFixExPoint"), &[EffectType::Usecardfixexpoint as i32]));
    (844, "DeadlyPoison") => DeadlyPoison, stat_read: OnTrigger,
        runtime: |context| super::deadly_poison::runtime_rule_ops(context.managers, context.subscriber, context.event?),
        supports: |args| matches!(args, [base, compound, cap]
            if *base > 0 && *compound >= 0 && *cap >= 0), wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(844, "DeadlyPoison"), &[EffectType::Deadlypoison as i32]));
    (759, "UseSkillToEnemy") => UseSkillToEnemy, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(759, "UseSkillToEnemy"), &[EffectType::None as i32]));
    (760, "ControlTeamInjuryCountRound") => ControlTeamInjuryCountRound,
        event: EventKind::HpLost, publication: BeforePublish,
        scoped_runtime: |context| super::control_team_injury_count_round::scoped_rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(760, "ControlTeamInjuryCountRound"), &[EffectType::Recordteaminjurycount as i32]));
    (764, "CareerRestraint") => CareerRestraint;
    (765, "CareerRatioFix") => CareerRatioFix, effect_time_subscription: false,
        supports: super::career_ratio_fix::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(765, "CareerRatioFix"), &[EffectType::None as i32]));
    (766, "AddToBuffEntity2") => AddToBuffEntity2,
        runtime: |context| super::add_to_buff_entity_2::rule_ops(context.subscriber, context.event?),
        supports: super::add_to_buff_entity_2::supports;
    (767, "AttrFixFromInjuryBank") => AttrFixFromInjuryBank, effect_time_subscription: false,
        stat_read: OnGrant, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(767, "AttrFixFromInjuryBank"), &[EffectType::None as i32]));
    (768, "InjuryLogback") => InjuryLogback, events: [EventKind::HpLost],
        runtime: |context| super::injury_bank::logback_rule_ops(context.managers, context.subscriber, context.event?),
        supports: |args| matches!(args, [settle_permille] if *settle_permille > 0), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(768, "InjuryLogback"), &[]));
    (769, "AbsorbHurt") => AbsorbHurt, effect_time_subscription: false, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(769, "AbsorbHurt"), &[EffectType::Absorbhurt as i32]));
    (770, "InjuryBank") => InjuryBank, event: EventKind::HpLost,
        transactions: [EventKind::BuffAdded],
        runtime: |context| super::injury_bank::runtime_rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        transaction: super::injury_bank::grant_transaction_rule_ops,
        supports: |args| matches!(args, [raw_attr, cap, skill, threshold, heal, store]
            if crate::engine::entity::attr::AttrId::from_raw(*raw_attr)
                == Some(crate::engine::entity::attr::AttrId::Hp)
                && *cap > 0 && *skill > 0 && *threshold > 0 && *heal > 0 && *store > 0), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(770, "InjuryBank"), &[EffectType::Storageinjury as i32]));
    (771, "MasterHalo") => MasterHalo, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(771, "MasterHalo"), &[EffectType::Masterhalo as i32]));
    (772, "SlaveHalo") => SlaveHalo, effect_time_subscription: false, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(772, "SlaveHalo"), &[EffectType::Slavehalo as i32]));
    (781, "MockTaunt") => MockTaunt, effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(781, "MockTaunt"), &[EffectType::Mocktaunt as i32]));
    (794, "ModifyMaxBurnLayers") => ModifyMaxBurnLayers, effect_time_subscription: false, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(794, "ModifyMaxBurnLayers"), &[EffectType::Bufftypenumlimitupdate as i32]));
    (901, "ModifyMaxBuffLayers") => ModifyMaxBuffLayers, effect_time_subscription: false,
        supports: |args| matches!(args, [buff_id, bonus] if *buff_id > 0 && *bonus > 0), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(901, "ModifyMaxBuffLayers"), &[EffectType::Bufftypenumlimitupdate as i32]));
    (1104, "ButterflyRecordSkill") => ButterflyRecordSkill,
        runtime: |context| super::butterfly_record_skill::rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: |args| matches!(args, [count, enchant_id, allowed @ ..]
            if *count > 0 && *enchant_id > 0 && !allowed.is_empty()), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1104, "ButterflyRecordSkill"), &[EffectType::None as i32]).with_initial_state(super::wire::InitialStateRule::ButterflyAllowedSkillKinds));
    (806, "ExPointOverflowBank") => ExPointOverflowBank,
        scoped_runtime: |context| super::ex_point_overflow_bank::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(806, "ExPointOverflowBank"), &[EffectType::Expointoverflowbank as i32]));
    (1008, "BanLostLife") => BanLostLife, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1008, "BanLostLife"), &[EffectType::None as i32]));
    (10001, "AdrenalineAddCard") => AdrenalineAddCard,
        runtime: |context| super::adrenaline_add_card::rule_ops(context.managers, context.subscriber, context.event?),
        supports: super::adrenaline_add_card::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(10001, "AdrenalineAddCard"), &[EffectType::None as i32]));
    (10000, "EzioBigSkill") => EzioBigSkill, effect_time_subscription: false, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(10000, "EzioBigSkill"), &[]));
    (10002, "AttrByHeroId") => AttrByHeroId, effect_time_subscription: false,
        supports: |args| matches!(args, [raw_attr, _, model_ids @ ..]
            if crate::engine::entity::attr::AttrId::from_raw(*raw_attr).is_some()
                && !model_ids.is_empty()
                && model_ids.iter().all(|model_id| *model_id > 0)), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(10002, "AttrByHeroId"), &[EffectType::Attr as i32]));
    (752, "AttrByDmgType") => AttrByDamageType, effect_time_subscription: false,
        supports: |args| matches!(args, [damage_type @ (1 | 2), raw_attr, _]
            if *damage_type > 0
                && crate::engine::entity::attr::AttrId::from_raw(*raw_attr).is_some()), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(752, "AttrByDmgType"), &[]));
    (10007, "AddAssassinateY") => AddAssassinateY, effect_time_subscription: false,
        supports: super::assassination::supports_source_bonus, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(10007, "AddAssassinateY"), &[EffectType::None as i32]));
    (702, "BuffReplace") => BuffReplace, effect_time_subscription: false, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(702, "BuffReplace"), &[EffectType::Buffreplace as i32]));
    (713, "ExSkillPointChange") => ExSkillPointChange, effect_time_subscription: false,
        supports: |args| matches!(args, [_]), state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(713, "ExSkillPointChange"), &[EffectType::Exskillpointchange as i32]));
    (405, "Disarm") => Disarm, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(405, "Disarm"), &[EffectType::Disarm as i32]));
    (406, "Forbid") => Forbid, effect_time_subscription: false,
        supports: |args| args.is_empty(), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(406, "Forbid"), &[]));
    (407, "Seal") => Seal, effect_time_subscription: false,
        supports: |args| args.is_empty(), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(407, "Seal"), &[]));
    (10005, "Provoke") => Provoke, effect_time_subscription: false, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(10005, "Provoke"), &[EffectType::None as i32]));
    (10004, "BeAttackedAssassinate") => BeAttackedAssassinate,
        event: EventKind::BeAttacked, frame: CausingFrame,
        runtime: |context| super::assassination::rule_ops(context.catalog, context.subscriber, context.event?),
        supports: super::assassination::supports_target_trigger, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(10004, "BeAttackedAssassinate"), &[EffectType::None as i32]));
    (10006, "BeatBackDependOnAttackMe") => BeatBackDependOnAttackMe,
        event: EventKind::SkillAction, phase: HitPassives, frame: CausingFrame, actor: OpposingTeam,
        runtime: |context| super::riposte::rule_ops(context.pool, context.subscriber, context.event?),
        supports: super::riposte::supports_dependent, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(10006, "BeatBackDependOnAttackMe"), &[EffectType::None as i32]));
    (815, "AddSpTempCard") => AddSpTempCard,
        scoped_runtime: |context| {
            let reserve_id = i64::from(context.pool.entity(context.subscriber.owner_uid)?.model_id);
            super::add_sp_temp_card::subscriber_rule_ops(context.subscriber, context.event?, reserve_id)
        },
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(815, "AddSpTempCard"), &[EffectType::None as i32]));
    (820, "AttrFromEntity") => AttrFromEntity, effect_time_subscription: false, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(820, "AttrFromEntity"), &[EffectType::Attr as i32]));
    (822, "LayerMasterHalo") => LayerMasterHalo, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(822, "LayerMasterHalo"), &[EffectType::Layermasterhalo as i32]));
    (825, "ConsumeBuffContinueChannel") => ConsumeBuffContinueChannel, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(825, "ConsumeBuffContinueChannel"), &[EffectType::None as i32]));
    (827, "Bullet") => Bullet,
        source: Applier,
        runtime: |context| super::bullet::rule_ops(context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(827, "Bullet"), &[EffectType::None as i32]));
    (834, "EachChangeAttr") => EachChangeAttr,
        transactions: [EventKind::BuffAdded, EventKind::BuffChanged, EventKind::BuffRemoved],
        publication: BeforePublish, frame: CausingFrame,
        transaction: super::each_change_attr::transaction_rule_ops, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(834, "EachChangeAttr"), &[EffectType::None as i32]));
    (861, "FixTempAttrByBuffLayer") => FixTempAttrByBuffLayer, stat_read: OnTrigger, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(861, "FixTempAttrByBuffLayer"), &[EffectType::None as i32]));
    (860, "MustCritAndFixTempAttr") => MustCritAndFixTempAttr,
        stat_read: OnTrigger,
        supports: super::must_crit_and_fix_temp_attr::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(860, "MustCritAndFixTempAttr"), &[EffectType::None as i32]));
    (863, "CreateAdditionalDamage") => CreateAdditionalDamage, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(863, "CreateAdditionalDamage"), &[EffectType::None as i32]));
    (865, "AddPassiveSkills") => AddPassiveSkills,
        supports: |args| matches!(args, [skill_id] if *skill_id > 0), state_consumer: true,
        wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(865, "AddPassiveSkills"), &[EffectType::None as i32]));
    (869, "ShellProcess") => ShellProcess, effect_time_subscription: false,
        events: [EventKind::ShellDeployed, EventKind::ShellRetrieved], frame: CausingFrame,
        runtime: |context| super::shell::rule_ops(context.managers, context.pool, context.determinism, context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(869, "ShellProcess"), &[EffectType::None as i32]));
    (870, "Shell") => Shell, event: EventKind::BeAttacked, frame: CausingFrame,
        runtime: |context| super::shell::rule_ops(context.managers, context.pool, context.determinism, context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(870, "Shell"), &[EffectType::None as i32]));
    (871, "ShellDebuff") => ShellDebuff, event: EventKind::BeAttacked, frame: CausingFrame,
        runtime: |context| super::shell::rule_ops(context.managers, context.pool, context.determinism, context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(871, "ShellDebuff"), &[EffectType::None as i32]));
    (872, "ShareHurt") => ShareHurt, frame: CausingFrame,
        runtime: |context| super::share_hurt::rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(872, "ShareHurt"), &[EffectType::None as i32]));
    (873, "ShellLock") => ShellLock, event: EventKind::ShellRetrieved, frame: CausingFrame,
        runtime: |context| super::shell::rule_ops(context.managers, context.pool, context.determinism, context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(873, "ShellLock"), &[EffectType::None as i32]));
    (875, "EmitterTag") => EmitterTag,
        events: [EventKind::ActionQueueCommitted, EventKind::ImpromptuResolved],
        transactions: [EventKind::EurekaChanged],
        setup: [BattleStart(0)], independent_setup: [BattleStart(0, 0)],
        frame: CausingFrame,
        scoped_runtime: |context| super::emitter_tag::rule_ops(context.managers, context.subscriber, context.event?),
        transaction: super::emitter_tag::transaction_rule_ops,
        setup_handler: |context| super::emitter_tag::setup_rule_ops(context.managers, &context.subscriber.feature, context.subscriber.stage),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(875, "EmitterTag"), &[EffectType::Emittertag as i32]));
    (876, "EmitterCareerChange") => EmitterCareerChange, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(876, "EmitterCareerChange"), &[EffectType::Emittercareerchange as i32]));
    (878, "EmitterNumChange") => EmitterNumChange, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(878, "EmitterNumChange"), &[EffectType::Emitternumchange as i32]));
    (879, "EmitterCardAllocateChange") => EmitterCardAllocateChange,
        supports: super::emitter_card_allocate_change::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(879, "EmitterCardAllocateChange"), &[EffectType::None as i32]));
    (880, "EmitterDamageUp") => EmitterDamageUp, effect_time_subscription: false, state_consumer: true;
    (881, "UseSkillTeamAddEmitterEnergy") => UseSkillTeamAddEmitterEnergy,
        publication: BeforePublish, frame: CausingFrame,
        runtime: |context| super::use_skill_team_add_emitter_energy::rule_ops(context.managers, context.subscriber, context.event?, context.catalog),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(881, "UseSkillTeamAddEmitterEnergy"), &[EffectType::Useskillteamaddemitterenergy as i32]));
    (882, "FixAttrTeamEnergy") => FixAttrTeamEnergy, effect_time_subscription: false,
        stat_read: OnGrant,
        supports: |args| super::fix_attr_team_energy::supports(BuffActKind::FixAttrTeamEnergy, args), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(882, "FixAttrTeamEnergy"), &[EffectType::Fixattrteamenergy as i32]));
    (883, "FixAttrTeamEnergyAndBuff") => FixAttrTeamEnergyAndBuff,
        runtime: |context| super::fix_attr_team_energy::rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: |args| super::fix_attr_team_energy::supports(BuffActKind::FixAttrTeamEnergyAndBuff, args), wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(883, "FixAttrTeamEnergyAndBuff"), &[EffectType::Fixattrteamenergyandbuff as i32]));
    (884, "AddToBuffEntity3") => AddBuffAfterAttack,
        event: EventKind::SkillAction, phase: AfterDamage,
        runtime: |context| super::add_buff_after_attack::rule_ops(context.subscriber, context.event?),
        supports: super::add_buff_after_attack::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(884, "AddToBuffEntity3"), &[EffectType::None as i32]));
    (889, "BeAttackByEmitterDamage") => BeAttackByEmitterDamage,
        runtime: |context| super::be_attack_by_emitter_damage::rule_ops(context.managers, context.subscriber, context.event?),
        supports: super::be_attack_by_emitter_damage::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(889, "BeAttackByEmitterDamage"), &[EffectType::None as i32]));
    (891, "AddSplitEmitterNum") => AddSplitEmitterNum, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(891, "AddSplitEmitterNum"), &[EffectType::Addsplitemitternum as i32]));
    (892, "AttackNumSplitEmitterNum") => AttackNumSplitEmitterNum, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(892, "AttackNumSplitEmitterNum"), &[EffectType::Conditionsplitemitternum as i32]));
    (893, "EmitterEnergyAddBuff") => EmitterEnergyAddBuff,
        event: EventKind::PlayerActionsResolved, frame: IndependentEvent,
        scoped_runtime: |context| super::emitter_energy_add_buff::rule_ops(context.managers, context.subscriber, context.event?),
        supports: super::emitter_energy_add_buff::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(893, "EmitterEnergyAddBuff"), &[EffectType::None as i32]));
    (920, "BuffAddActLimit") => BuffAddActLimit, effect_time_subscription: false,
        supports: super::add_action_point::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(920, "BuffAddActLimit"), &[EffectType::None as i32]));
    (922, "AttrAndLayerAttr") => AttrAndLayerAttr, effect_time_subscription: false,
        supports: |args| args.len() >= 3
            && args.len() % 3 == 0
            && args.chunks_exact(3).all(|values|
                crate::engine::entity::attr::AttrId::from_raw(values[0]).is_some()), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(922, "AttrAndLayerAttr"), &[EffectType::None as i32]));
    (923, "AddCardCastChannel") => AddCardCastChannel,
        event: EventKind::ActionQueueCommitted,
        transactions: [EventKind::BuffAdded, EventKind::BuffRemoved],
        publication: AfterPublish,
        runtime: |context| super::add_card_cast_channel::rule_ops(context.managers, context.subscriber, context.event?),
        transaction: super::add_card_cast_channel::transaction_rule_ops,
        supports: super::add_card_cast_channel::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(923, "AddCardCastChannel"), &[]));
    (924, "EmitterRendTarget") => EmitterRendTarget, effect_time_subscription: false,
        supports: super::emitter_rend_target::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(924, "EmitterRendTarget"), &[]));
    (926, "ExPointAddByHit") => ExPointAddByHit, runtime_marker: AfterFirstChange(Source),
        runtime: |context| super::ex_point_add_by_hit::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(926, "ExPointAddByHit"), &[EffectType::None as i32]));
    (927, "AddBuffByOtherExSkill") => AddBuffByOtherExSkill,
        transactions: [EventKind::BuffAdded],
        frame: CausingFrame,
        runtime: |context| super::add_buff_by_other_ex_skill::rule_ops(context.catalog, context.subscriber, context.event?),
        transaction: super::add_buff_by_other_ex_skill::grant_transaction_rule_ops,
        supports: super::add_buff_by_other_ex_skill::supports;
    (932, "FixAttrBySubBuffLayer") => FixAttrBySubBuffLayer,
        supports: super::fix_attr_by_sub_buff_layer::supports, state_consumer: true,
        wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(932, "FixAttrBySubBuffLayer"), &[EffectType::None as i32]));
    (933, "SubBuff") => SubBuff, effect_time_subscription: false,
        supports: |args| matches!(args, [buff_id] if *buff_id > 0), state_consumer: true,
        wire: (super::wire::BuffActWireDefinition::add_refresh(DefinitionKey::new(933, "SubBuff"), &[EffectType::None as i32]));
    (928, "AddToTarget") => AddToAttackTargets,
        event: EventKind::SkillAction, phase: AfterDamage,
        runtime: |context| super::add_to_target::rule_ops(context.subscriber, context.event?, context.catalog, context.pool),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(928, "AddToTarget"), &[EffectType::Addtotarget as i32]));
    (929, "AddCardRecordByRound") => AddCardRecordByRound,
        event: EventKind::ActionQueueCommitted, publication: BeforePublish,
        frame: IndependentEvent,
        runtime: |context| super::card_record::rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(929, "AddCardRecordByRound"), &[EffectType::Addcardrecordbyround as i32]));
    (951, "CardNotCalSize") => CardNotCalSize, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(951, "CardNotCalSize"), &[EffectType::None as i32]));
    (1137, "EntityExSkillNotCalSize") => EntityExSkillNotCalSize, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1137, "EntityExSkillNotCalSize"), &[EffectType::None as i32]));
    (953, "BloodPoolTag") => BloodPoolTag,
        events: [EventKind::HpLost, EventKind::GaugeChanged],
        setup: [BattleStart(0), Unconditional(0), RoundStart(-1)], independent_setup: [BattleStart(0, 1), Unconditional(0, 0), RoundStart(-1, 0)],
        publications: [EventKind::GaugeChanged => BeforePublish],
        frame: CausingFrame,
        scoped_runtime: |context| super::blood_pool::tag::rule_ops(context.managers, context.subscriber, context.event?),
        setup_handler: |context| super::blood_pool::tag::setup_rule_ops(context.managers, context.catalog, &context.subscriber.feature, context.subscriber.stage),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(953, "BloodPoolTag"), &[EffectType::None as i32]));
    (955, "AttrByShield") => AttrByShield, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(955, "AttrByShield"), &[EffectType::Attr as i32]));
    (1005, "AttrOnlyCalDamageReplaceAttrADCreator") => AttrOnlyCalDamageReplaceAttrAdCreator,
        attack_replacement: super::attr_only_cal_damage_replace_attr_ad_creator::additional_damage_attack_replacement, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1005, "AttrOnlyCalDamageReplaceAttrADCreator"), &[EffectType::None as i32]));
    (1007, "AttrOnlyCalDamageReplaceAttr") => AttrOnlyCalDamageReplaceAttr,
        attack_replacement: super::attr_only_cal_damage_replace_attr_ad_creator::skill_attack_replacement, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1007, "AttrOnlyCalDamageReplaceAttr"), &[EffectType::None as i32]));
    (1009, "BloodValueUseSkill") => BloodValueUseSkill, event: EventKind::GaugeChanged,
        runtime: |context| super::blood_pool::value_use_skill::rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        supports: super::blood_pool::value_use_skill::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1009, "BloodValueUseSkill"), &[EffectType::None as i32]));
    (1010, "DyingHealDisperse1") => DyingHealDisperse1,
        runtime: |context| super::revive::rule_ops(context.managers, context.subscriber, context.event?),
        supports: super::revive::supports_dying_heal, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1010, "DyingHealDisperse1"), &[EffectType::None as i32]));
    (1011, "CureUpByLostHp") => CureUpByLostHp, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1011, "CureUpByLostHp"), &[EffectType::Cureupbylosthp as i32]));
    (1019, "LostHpCountAddBuff") => LostHpCountAddBuff,
        events: [EventKind::HpLost],
        runtime: |context| super::lost_hp_count_add_buff::rule_ops(context.managers, context.subscriber, context.event?),
        supports: super::lost_hp_count_add_buff::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1019, "LostHpCountAddBuff"), &[EffectType::None as i32]));
    (1021, "BloodPoolCountAddExPoint") => BloodPoolCountAddExPoint,
        transactions: [EventKind::SkillAction],
        setup: [RoundStart(4)],
        timing: AfterAction, runtime_marker: BeforeChanges(Owner),
        transaction: super::blood_pool::count_add_ex_point::event_rule_ops,
        setup_handler: |context| super::blood_pool::count_add_ex_point::setup_rule_ops(context.managers, &context.subscriber.feature),
        supports: |args| matches!(args, [threshold, amount] if *threshold > 0 && *amount > 0), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1021, "BloodPoolCountAddExPoint"), &[EffectType::None as i32]));
    (1022, "AttrOnlyCalDamageHpReplaceAttackCalSkillDamage") => AttrOnlyCalDamageHpReplaceAttackCalSkillDamage,
        effect_time_subscription: false,
        supports: super::attr_only_cal_damage_hp_replace_attack::supports,
        attack_replacement: super::attr_only_cal_damage_hp_replace_attack::skill_attack_replacement, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1022, "AttrOnlyCalDamageHpReplaceAttackCalSkillDamage"), &[EffectType::None as i32]));
    (1006, "NuoDiKaCastChannel") => NuoDiKaCastChannel,
        scoped_runtime: |context| super::nuo_di_ka_cast_channel::scoped_rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        supports: super::nuo_di_ka_cast_channel::supports, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1006, "NuoDiKaCastChannel"), &[EffectType::None as i32]).with_pre_add(super::wire::WireEffect { effect_type: EffectType::Nuodikarandomattacknum as i32, effect_num: 0, effect_num1: 1 }));
    (1023, "LostHpAddExtraBloodPoolValue") => LostHpAddExtraBloodPoolValue,
        settlement: After,
        runtime: |context| Some(super::lost_hp_add_extra_blood_pool_value::rule_ops(context.managers, context.subscriber)),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1023, "LostHpAddExtraBloodPoolValue"), &[EffectType::None as i32]));
    (1024, "MonitorContinueChannel") => MonitorContinueChannel,
        events: [EventKind::AllyAction], source: Owner, team: Opposing,
        scoped_runtime: |context| super::monitor_continue_channel::scoped_rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: |args| args.get(1).is_some_and(|skill_id| *skill_id > 0), wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1024, "MonitorContinueChannel"), &[EffectType::None as i32]));
    (1025, "LifeAttackFixRate") => LifeAttackFixRate,
        effect_time_subscription: false,
        supports: super::life_attack_fix_rate::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1025, "LifeAttackFixRate"), &[EffectType::None as i32]));
    (1026, "CreateMaxHpAdditionalDamageAndRemove") => CreateMaxHpAdditionalDamageAndRemove, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1026, "CreateMaxHpAdditionalDamageAndRemove"), &[EffectType::None as i32]));
    (1027, "AddBuffByChargingTimes") => AddBuffByChargingTimes,
        runtime: |context| Some(super::add_buff_by_charging_times::rule_ops(context.managers, context.pool, context.subscriber)),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1027, "AddBuffByChargingTimes"), &[EffectType::None as i32]));
    (1028, "RealDamageKill") => RealDamageKill, event: super::real_damage_kill::EVENT,
        runtime: |context| super::real_damage_kill::rule_ops(context.managers, context.pool, context.subscriber),
        supports: super::real_damage_kill::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1028, "RealDamageKill"), &[]).with_initial_state(super::wire::InitialStateRule::CurrentHpPermille));
    (1029, "AddAttrByOtherBuffLayer") => AddAttrByOtherBuffLayer, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1029, "AddAttrByOtherBuffLayer"), &[EffectType::Attr as i32]));
    (945, "CritRateAlter2") => CritRateAlter2,
        runtime: |context| super::crit_rate_alter2::supports(&context.subscriber.args).then(Vec::new),
        supports: super::crit_rate_alter2::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(945, "CritRateAlter2"), &[EffectType::None as i32]));
    (1071, "CritRateAlter2") => CritRateAlter2,
        effect_time_subscription: false,
        supports: super::crit_rate_alter2::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1071, "CritRateAlter2"), &[EffectType::None as i32]));
    (1030, "CritRateAlterByOtherBuff") => CritRateAlterByOtherBuff,
        runtime: |context| super::crit_rate_alter_by_other_buff::supports(&context.subscriber.args).then(Vec::new),
        supports: super::crit_rate_alter_by_other_buff::supports, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1030, "CritRateAlterByOtherBuff"), &[EffectType::None as i32]));
    (1003, "SpecialCountContinueChannelBuff") => SpecialCountContinueChannelBuff,
        runtime: |context| Some(super::special_count_continue_channel::rule_ops(context.managers, context.subscriber)),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1003, "SpecialCountContinueChannelBuff"), &[EffectType::None as i32]));
    (1002, "SpecialCountCastChannel") => SpecialCountCastChannel,
        runtime: |context| super::special_count_cast_channel::rule_ops(context.subscriber, context.event?, context.catalog),
        supports: |args| matches!(args, [skill_id, ..] if *skill_id > 0);
    (1004, "AddAttrBySpecialCount") => AddAttrBySpecialCount;
    (1031, "ConsumeBuffAddBuffContinueChannel") => ConsumeBuffAddBuffContinueChannel;
    (1032, "FixElectricUpgrade") => FixElectricUpgrade, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1032, "FixElectricUpgrade"), &[EffectType::None as i32]));
    (1033, "TransferEnergyBuff") => TransferEnergyBuff,
        effect_time_subscription: false, events: [EventKind::ExPointOverflow],
        scoped_runtime: |context| super::transfer_energy_buff::rule_ops(context.managers, context.pool, context.subscriber, context.event?),
        supports: super::transfer_energy_buff::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1033, "TransferEnergyBuff"), &[EffectType::None as i32]));
    (1034, "AddBuffToEnter") => AddBuffToEnter,
        effect_time_subscription: false, supports: super::add_buff_to_enter::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1034, "AddBuffToEnter"), &[EffectType::None as i32]));
    (946, "BigSkillNoUseActPoint") => BigSkillNoUseActPoint,
        effect_time_subscription: false, events: [EventKind::AllyAction],
        runtime: |context| super::big_skill_no_use_action_point::rule_ops(context.managers, context.catalog, context.subscriber, context.event?),
        supports: super::big_skill_no_use_action_point::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(946, "BigSkillNoUseActPoint"), &[EffectType::None as i32]));
    (1036, "AddAttrByOtherBuffLayer") => AddAttrByOtherBuffLayer, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1036, "AddAttrByOtherBuffLayer"), &[EffectType::None as i32, EffectType::Attr as i32]));
    (1041, "RaspberryBigSkill") => RaspberryBigSkill,
        effect_time_subscription: false, supports: |_| true, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1041, "RaspberryBigSkill"), &[EffectType::None as i32]));
    (1042, "Raspberry") => Raspberry, events: [EventKind::BuffRemoved],
        runtime: |context| super::raspberry::rule_ops(context.managers, context.subscriber, context.event?),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1042, "Raspberry"), &[EffectType::None as i32]).with_max_hp(1, 1042));
    (1043, "Revive") => Revive,
        runtime: |context| super::revive::rule_ops(context.managers, context.subscriber, context.event?),
        supports: super::revive::supports, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1043, "Revive"), &[EffectType::Cure as i32]));
    (1048, "Radiance") => Radiance, stat_read: OnTrigger,
        runtime: |context| Some(super::damage_over_time::damage_rule_ops(context.managers, context.pool, context.determinism, context.subscriber)),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1048, "Radiance"), &[EffectType::Radiance as i32]));
    (1049, "CrystalNotifySelect") => CrystalNotifySelect, effect_time_subscription: false,
        supports: |args| matches!(args, [total, per_crystal, ..]
            if *total > 0 && *per_crystal > 0), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1049, "CrystalNotifySelect"), &[EffectType::None as i32]).with_initial_state(super::wire::InitialStateRule::CrystalSelection));
    (10030, "TwinsNotifySelect") => ConduitCardSelection, effect_time_subscription: false,
        supports: super::conduit_select::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(10030, "TwinsNotifySelect"), &[]).with_initial_state(super::wire::InitialStateRule::ConduitCardSelection));
    (1050, "HeatScaleUseSkill") => HeatScaleUseSkill,
        scoped_runtime: |context| Some(super::heat_scale_use_skill::rule_ops(context.managers, context.catalog, context.subscriber)),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1050, "HeatScaleUseSkill"), &[EffectType::None as i32]).with_initial_state(super::wire::InitialStateRule::HeatScale));
    (1051, "CrystalAddBuff") => CrystalAddBuff,
        event: EventKind::SkillAction, phase: AfterDamage, settlement: After,
        scoped_runtime: |context| super::crystal_add_buff::scoped_rule_ops(context.managers, context.subscriber, context.event?),
        supports: |args| matches!(args, [buff_id, blue, purple, green_two, green_three, ..]
            if *buff_id > 0 && [blue, purple, green_two, green_three].iter().all(|value| **value >= 0)), wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1051, "CrystalAddBuff"), &[EffectType::None as i32]));
    (1052, "HeatScaleTag") => HeatScaleTag,
        effect_time_subscription: false,
        events: [EventKind::BuffAdded, EventKind::BuffChanged, EventKind::GaugeChanged],
        setup: [BattleStart(0), BuffGate(0), RoundStart(3)], mechanic_setup: [BuffGate(0)],
        root_mechanic_setup: [RoundStart(3)],
        independent_setup: [BattleStart(0, 1)],
        publications: [EventKind::GaugeChanged => BeforePublish],
        frame: CausingFrame,
        scoped_runtime: |context| super::heat_scale_tag::rule_ops(context.managers, context.subscriber, context.event?),
        setup_handler: |context| super::heat_scale_tag::setup_rule_ops(context.managers, context.catalog, &context.subscriber.feature, context.subscriber.stage),
        supports: |_| true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1052, "HeatScaleTag"), &[EffectType::None as i32]));
    (1053, "AttrByHeatScale") => AttrByHeatScale, trigger_child_uid: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1053, "AttrByHeatScale"), &[EffectType::None as i32]));
    (1062, "HeatScaleDecrCounter") => HeatScaleDecrCounter, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1062, "HeatScaleDecrCounter"), &[EffectType::None as i32]));
    (1069, "ImmunityTimes") => ImmunityTimes,
        effect_time_subscription: false,
        supports: |args| matches!(args, [status] if *status > 0), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1069, "ImmunityTimes"), &[EffectType::None as i32]));
    (1070, "HeatScaleAddFix") => HeatScaleAddFix,
        effect_time_subscription: false,
        supports: |args| matches!(args, [value] if *value != 0), state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1070, "HeatScaleAddFix"), &[EffectType::None as i32]));
    (1075, "CardLimitAdd") => CardLimitAdd, effect_time_subscription: false,
        supports: |args| matches!(args, [delta] if *delta != 0), state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1075, "CardLimitAdd"), &[EffectType::None as i32]));
    (1081, "EmitterFixSubTargetsDamageReduceRate") => EmitterFixSubTargetsDamageReduceRate;
    (1114, "HeatScaleBurnAddFix") => HeatScaleBurnAddFix;
    (1125, "TeamShareShield") => TeamShareShield, effect_time_subscription: false,
        supports: super::team_share_shield::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1125, "TeamShareShield"), &[]));
    (1126, "TeamImmunityTimes") => TeamImmunityTimes,
        effect_time_subscription: false,
        setup: [RoundStart(2)],
        setup_handler: |context| super::team_immunity_times::setup_rule_ops(context.managers, &context.subscriber.feature, context.subscriber.stage),
        supports: super::team_immunity_times::supports, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1126, "TeamImmunityTimes"), &[EffectType::None as i32]).with_initial_state(super::wire::InitialStateRule::SecondArgument));
    (1127, "TeamExElectricTransConsumeValueAttr") => TeamExElectricTransConsumeValueAttr,
        effect_time_subscription: false, stat_read: OnGrant,
        supports: super::electric_transform::supports_team_attribute, state_consumer: true, wire: (super::wire::BuffActWireDefinition::all(DefinitionKey::new(1127, "TeamExElectricTransConsumeValueAttr"), &[]).with_initial_state(super::wire::InitialStateRule::GrantValue));
    (1128, "RecordTeamExElectricTransConsumeValue") => RecordTeamExElectricTransConsumeValue,
        effect_time_subscription: false,
        supports: super::electric_transform::supports_record, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1128, "RecordTeamExElectricTransConsumeValue"), &[EffectType::None as i32]));
    (1129, "ExtraValueElectricTransform") => ExtraValueElectricTransform,
        effect_time_subscription: false,
        supports: super::electric_transform::supports_extra_value, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1129, "ExtraValueElectricTransform"), &[EffectType::None as i32]));
    (1130, "DeviceCostReduce") => DeviceCostReduce,
        effect_time_subscription: false,
        supports: super::device_cost_reduce::supports, state_consumer: true, wire: (super::wire::BuffActWireDefinition::add(DefinitionKey::new(1130, "DeviceCostReduce"), &[EffectType::None as i32]));
}

pub fn definitions() -> impl Iterator<Item = &'static BuffActDefinition> {
    DEFINITIONS.iter()
}

pub fn transaction_definitions(
    event: EventKind,
) -> impl Iterator<Item = &'static BuffActDefinition> {
    definitions().filter(move |definition| definition.transaction.events.contains(&event))
}

/// Exact buff-act support and execution gateway.
/// `effectTime` selects an event lane but never substitutes for this key.
pub fn find(opcode: i32, type_name: &str) -> Option<&'static BuffActDefinition> {
    definitions().find(|definition| definition.key.matches(opcode, type_name))
}

pub fn kind(opcode: i32, type_name: &str) -> Option<BuffActKind> {
    find(opcode, type_name).map(|definition| definition.kind)
}

pub fn reserves_trigger_child_uid(key: DefinitionKey) -> bool {
    find(key.opcode, key.type_name)
        .is_some_and(|definition| definition.runtime.reserves_trigger_child_uid)
}

pub fn runtime_marker(key: DefinitionKey) -> Option<RuntimeMarker> {
    find(key.opcode, key.type_name)?.runtime.marker
}

pub fn runtime_event(act_id: i32, act_type: &str, effect_time: i32) -> Option<EventKind> {
    if let Some(definition) = find(act_id, act_type) {
        if !definition.runtime.effect_time_subscription {
            return None;
        }
        if let Some(event) = definition.runtime.event_override {
            return Some(event);
        }
    }
    match super::effect_time::classify(effect_time) {
        super::effect_time::BuffActEvent::Runtime(event) => Some(event),
        _ => None,
    }
}

pub fn runtime_phase(
    act_id: i32,
    act_type: &str,
    effect_time: i32,
    event: EventKind,
) -> Option<crate::engine::skill::action::SkillPhase> {
    let definition = find(act_id, act_type)?;
    if definition.runtime.event_override == Some(event)
        && let Some(phase) = definition.runtime.phase_override
    {
        return Some(phase);
    }
    (event == EventKind::SkillAction)
        .then(|| super::effect_time::find(effect_time)?.duration_phase)
        .flatten()
}

pub fn subscribes_to_event(
    act_id: i32,
    act_type: &str,
    effect_time: i32,
    event: EventKind,
) -> bool {
    find(act_id, act_type).is_some_and(|definition| definition.runtime.events.contains(&event))
        || runtime_event(act_id, act_type, effect_time) == Some(event)
}

pub fn runtime_publication(act_id: i32, act_type: &str, event: EventKind) -> PublicationPhase {
    find(act_id, act_type)
        .map(|definition| {
            definition
                .runtime
                .publications
                .iter()
                .find_map(|(registered, phase)| (*registered == event).then_some(*phase))
                .unwrap_or(definition.runtime.publication)
        })
        .unwrap_or_default()
}

pub fn runtime_team_scope(act_id: i32, act_type: &str) -> RuntimeTeamScope {
    find(act_id, act_type)
        .map(|definition| definition.runtime.team_scope)
        .unwrap_or_default()
}

pub fn runtime_actor_scope(act_id: i32, act_type: &str) -> RuntimeActorScope {
    find(act_id, act_type)
        .map(|definition| definition.runtime.actor_scope)
        .unwrap_or_default()
}

pub fn runtime_settlement_phase(act_id: i32, act_type: &str) -> RuntimeSettlementPhase {
    find(act_id, act_type)
        .map(|definition| definition.runtime.settlement_phase)
        .unwrap_or_default()
}

pub fn linked_rule_ops(
    owner_uid: i64,
    opcode: i32,
    type_name: &str,
    args: &[i32],
) -> Option<Vec<RuleOp>> {
    super::use_skill::linked_for(owner_uid, opcode, type_name, args)
        .map(|plan| vec![RuleOp::Skill(plan.into())])
}

pub fn destination(opcode: i32, type_name: &str, args: &[i32]) -> Option<BuffActDestination> {
    let definition = find(opcode, type_name)?;
    if definition.supports.is_some_and(|supports| !supports(args)) {
        return None;
    }
    definition.destination().or_else(|| {
        linked_rule_ops(0, opcode, type_name, args)
            .is_some()
            .then_some(BuffActDestination::LinkedSkill)
    })
}

pub fn has_destination(opcode: i32, type_name: &str, args: &[i32]) -> bool {
    destination(opcode, type_name, args).is_some()
}

#[cfg(test)]
mod tests;
