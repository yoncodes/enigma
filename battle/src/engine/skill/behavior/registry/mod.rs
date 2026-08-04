use crate::engine::event::kind::EventKind;
use crate::engine::round::modifier::RoundModifiers;
use crate::engine::skill::{
    action::SkillPhase,
    behavior::{AttackModifierContext, BehaviorOpContext, classify::BehaviorKind},
    effect::ParsedBehavior,
    rule::{DefinitionKey, RuleReferences, SetupStage, output::RuleOp},
};

pub type BehaviorPhase = SkillPhase;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FireCountMode {
    Repeat,
    Transfer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetEmissionMode {
    Each,
    Once,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillDestinationMode {
    Repeat,
    Unique,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputOwner {
    Skill,
    Parent,
    CausingEvent,
    SetupParent,
}

impl OutputOwner {
    pub fn resolve(self, event_triggered: bool, setup_triggered: bool) -> Self {
        match self {
            Self::CausingEvent if event_triggered => Self::Parent,
            Self::CausingEvent => Self::Skill,
            Self::SetupParent if setup_triggered => Self::Parent,
            Self::SetupParent => Self::Skill,
            owner => owner,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardPlayRole {
    Action,
    QueuePreparation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionRouteOverride {
    Trigger {
        key: DefinitionKey,
        event: EventKind,
        phase: Option<SkillPhase>,
    },
    Setup {
        key: DefinitionKey,
        stage: SetupStage,
        priority: i32,
    },
}

pub type QueuePreparationCollector = fn(i32, &ParsedBehavior) -> Option<Vec<RuleOp>>;

pub struct BehaviorFireCountContext<'a> {
    pub managers: &'a crate::engine::manager::BattleManagers,
    pub source_team: i32,
}

pub struct BehaviorDefinition {
    pub key: DefinitionKey,
    pub kind: BehaviorKind,
    pub phase: BehaviorPhase,
    pub emit_ops: for<'a> fn(BehaviorOpContext<'a>, &ParsedBehavior) -> Option<Vec<RuleOp>>,
    pub collect_attack_modifier:
        Option<for<'a> fn(AttackModifierContext<'a>, &ParsedBehavior) -> bool>,
    pub collect_round_modifier: Option<fn(&ParsedBehavior) -> Option<RoundModifiers>>,
    pub round_modifier_only: bool,
    pub collect_queue_preparation: Option<QueuePreparationCollector>,
    pub destination: bool,
    pub fire_count_mode: FireCountMode,
    pub resolve_fire_count: for<'a> fn(BehaviorFireCountContext<'a>, &ParsedBehavior, i32) -> i32,
    pub target_emission_mode: TargetEmissionMode,
    pub skill_destination_mode: SkillDestinationMode,
    pub output_owner: OutputOwner,
    pub output_owner_for: fn(&ParsedBehavior, usize) -> Option<OutputOwner>,
    pub references: fn(&ParsedBehavior) -> RuleReferences,
    pub card_play_role: CardPlayRole,
    pub condition_route_override: Option<ConditionRouteOverride>,
    pub supports: Option<fn(&ParsedBehavior) -> bool>,
}

pub trait BehaviorHandler {
    const VALIDATES_ARGUMENTS: bool = false;

    fn supports(_: &ParsedBehavior) -> bool {
        false
    }

    fn references(_: &ParsedBehavior) -> RuleReferences {
        RuleReferences::default()
    }

    fn emit_ops(_: BehaviorOpContext<'_>, _: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        None
    }

    fn collect_attack_modifier(
        context: AttackModifierContext<'_>,
        behavior: &ParsedBehavior,
    ) -> bool {
        Self::emit_ops(context.operation, behavior).is_some()
    }

    fn collect_round_modifier(_: &ParsedBehavior) -> Option<RoundModifiers> {
        None
    }

    fn collect_queue_preparation(_: i32, _: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        None
    }

    fn output_owner(_: &ParsedBehavior, _: usize) -> Option<OutputOwner> {
        None
    }

    fn resolve_fire_count(
        _: BehaviorFireCountContext<'_>,
        _: &ParsedBehavior,
        fallback: i32,
    ) -> i32 {
        fallback
    }
}

pub const fn definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        key: DefinitionKey::new(opcode, type_name),
        kind,
        phase,
        emit_ops: H::emit_ops,
        collect_attack_modifier: None,
        collect_round_modifier: None,
        round_modifier_only: false,
        collect_queue_preparation: None,
        destination: false,
        fire_count_mode: FireCountMode::Repeat,
        resolve_fire_count: H::resolve_fire_count,
        target_emission_mode: TargetEmissionMode::Each,
        skill_destination_mode: SkillDestinationMode::Repeat,
        output_owner: OutputOwner::Skill,
        output_owner_for: H::output_owner,
        references: H::references,
        card_play_role: CardPlayRole::Action,
        condition_route_override: None,
        supports: if H::VALIDATES_ARGUMENTS {
            Some(H::supports)
        } else {
            None
        },
    }
}

pub const fn with_condition_route(
    mut definition: BehaviorDefinition,
    route: ConditionRouteOverride,
) -> BehaviorDefinition {
    definition.condition_route_override = Some(route);
    definition
}

pub const fn with_argument_parser(
    mut definition: BehaviorDefinition,
    supports: fn(&ParsedBehavior) -> bool,
) -> BehaviorDefinition {
    definition.supports = Some(supports);
    definition
}

pub mod arguments {
    use crate::engine::skill::effect::ParsedBehavior;

    pub fn none(behavior: &ParsedBehavior) -> bool {
        behavior.args.is_empty()
    }

    pub fn at_least_one(behavior: &ParsedBehavior) -> bool {
        !behavior.args.is_empty()
    }

    pub fn exactly_two(behavior: &ParsedBehavior) -> bool {
        behavior.args.len() == 2
    }

    pub fn exactly_three(behavior: &ParsedBehavior) -> bool {
        behavior.args.len() == 3
    }

    pub fn exactly_four(behavior: &ParsedBehavior) -> bool {
        behavior.args.len() == 4
    }
}

pub const fn unique_skill_destination_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        destination: true,
        skill_destination_mode: SkillDestinationMode::Unique,
        ..definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn queue_preparation_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        destination: true,
        card_play_role: CardPlayRole::QueuePreparation,
        collect_queue_preparation: Some(H::collect_queue_preparation),
        ..definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn once_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        target_emission_mode: TargetEmissionMode::Once,
        ..definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn once_destination_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        destination: true,
        ..once_definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn parent_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        output_owner: OutputOwner::Parent,
        ..definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn parent_destination_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        destination: true,
        ..parent_definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn setup_parent_destination_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        destination: true,
        output_owner: OutputOwner::SetupParent,
        ..definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn causing_event_destination_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        destination: true,
        output_owner: OutputOwner::CausingEvent,
        ..definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn transfer_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        destination: true,
        fire_count_mode: FireCountMode::Transfer,
        ..definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn destination_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        destination: true,
        ..definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn modifier_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        destination: true,
        collect_attack_modifier: Some(H::collect_attack_modifier),
        ..definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn round_modifier_only_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        destination: true,
        collect_round_modifier: Some(H::collect_round_modifier),
        round_modifier_only: true,
        ..definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn round_modifier_with_output_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        destination: true,
        collect_round_modifier: Some(H::collect_round_modifier),
        ..definition::<H>(opcode, type_name, kind, phase)
    }
}

pub const fn aggregated_destination_definition<H: BehaviorHandler>(
    opcode: i32,
    type_name: &'static str,
    kind: BehaviorKind,
    phase: BehaviorPhase,
) -> BehaviorDefinition {
    BehaviorDefinition {
        destination: true,
        fire_count_mode: FireCountMode::Transfer,
        ..definition::<H>(opcode, type_name, kind, phase)
    }
}

macro_rules! behavior_definitions {
    ($([$opcode:expr] $type_name:literal => $handler:ty, $kind:ident, $phase:ident, $mode:ident $(, @route($route:expr))? $(, $supports:path)?);+ $(;)?) => {
        pub const DEFINITIONS: &[BehaviorDefinition] =
            &[$(behavior_definitions!(@maybe_support $mode, $handler, $opcode, $type_name, $kind, $phase $(, @route($route))? $(, $supports)?)),+];
    };
    (@maybe_support $mode:ident, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident, @route($route:expr), $supports:path) => {
        $crate::engine::skill::behavior::registry::with_argument_parser(
            $crate::engine::skill::behavior::registry::with_condition_route(
                behavior_definitions!(@definition $mode, $handler, $opcode, $type_name, $kind, $phase),
                $route,
            ),
            $supports,
        )
    };
    (@maybe_support $mode:ident, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident, @route($route:expr)) => {
        $crate::engine::skill::behavior::registry::with_condition_route(
            behavior_definitions!(@definition $mode, $handler, $opcode, $type_name, $kind, $phase),
            $route,
        )
    };
    (@maybe_support $mode:ident, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident, $supports:path) => {
        $crate::engine::skill::behavior::registry::with_argument_parser(
            behavior_definitions!(@definition $mode, $handler, $opcode, $type_name, $kind, $phase),
            $supports,
        )
    };
    (@maybe_support $mode:ident, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        behavior_definitions!(@definition $mode, $handler, $opcode, $type_name, $kind, $phase)
    };
    (@definition plain, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition destination, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::destination_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition unique_skill_destination, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::unique_skill_destination_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition queue_preparation, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::queue_preparation_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition once_destination, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::once_destination_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition parent, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::parent_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition parent_destination, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::parent_destination_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition setup_parent_destination, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::setup_parent_destination_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition causing_event_destination, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::causing_event_destination_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition transfer, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::transfer_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition modifier, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::modifier_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition round_modifier_only, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::round_modifier_only_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition round_modifier_with_output, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::round_modifier_with_output_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
    (@definition aggregated_destination, $handler:ty, $opcode:expr, $type_name:literal, $kind:ident, $phase:ident) => {
        $crate::engine::skill::behavior::registry::aggregated_destination_definition::<$handler>($opcode, $type_name, $crate::engine::skill::behavior::classify::BehaviorKind::$kind, $crate::engine::skill::behavior::registry::BehaviorPhase::$phase)
    };
}

behavior_definitions! {
    [20002] "AddExPoint" => super::resource::Handler, AddExPoint, AfterDamage, aggregated_destination, arguments::at_least_one;
    [100004] "AddAdrenalineExPoint" => super::resource::Handler, AddAdrenalineExPoint, AfterDamage, destination, super::resource::supports_ex_point_gain;
    [100014] "EzioAddSynchronization" => super::resource::Handler, AddSynchronization, AfterDamage, destination;
    [10010] "AttrFixExPoint" => super::resource::Handler, AttrFixExPoint, Immediate, destination;
    [30001] "DelExPoint" => super::resource::Handler, DelExPoint, AfterDamage, destination, super::resource::supports_ex_point_loss;
    [30007] "DelExPoint" => super::resource::Handler, DelExPoint, AfterDamage, destination;
    [30013] "DelExPointNotImmunity" => super::resource::Handler, DelExPointNotImmunity, AfterDamage, destination;
    [30011] "AbsorbExPoint" => super::resource::Handler, AbsorbExPoint, Immediate, destination;
    [20011] "AverageLife" => super::resource::Handler, AverageLife, Immediate, destination, super::resource::supports_average_life;
    [50017] "ChangePower" => super::resource::Handler, ChangePower, Immediate, destination, super::resource::supports_power_change;
    [50037] "ChangePower" => super::resource::Handler, ChangePower, Immediate, destination;
    [60144] "RecoverPower" => super::resource::Handler, RecoverPower, Immediate, destination, super::resource::supports_recover_power;
    [60125] "RecoverPowerAndDelCardsUseSkill" => super::resource::Handler, RecoverPowerAndDelCardsUseSkill, Immediate, destination, super::resource::supports_recover_power_and_cast_cards;
    [60187] "AddPowerByCritCount" => super::resource::Handler, AddPowerByCritCount, Immediate, destination, super::resource::supports_power_by_critical_count;
    [60115] "TotalSkillRankToPower" => super::resource::Handler, TotalSkillRankToPower, Immediate, destination, super::resource::supports_total_skill_rank_power;
    [60152] "AddEmitterEnergy" => super::resource::Handler, AddEmitterEnergy, Immediate, destination, super::resource::supports_emitter_energy;
    [60153] "AddTeamEnergy" => super::resource::Handler, AddTeamEnergy, Immediate, setup_parent_destination, super::resource::supports_team_energy;
    [60154] "AddRedOrBlueCount" => super::resource::Handler, AddRedOrBlueCount, Immediate, destination, super::resource::supports_red_or_blue_count;
    [60291] "AddDevicePower" => super::resource::Handler, AddConduitPower, Immediate, destination, super::resource::supports_conduit_power;
    [60292] "AddDeviceExPoint" => super::resource::Handler, AddConduitExPoint, Immediate, setup_parent_destination, super::resource::supports_ex_point_gain;
    [60293] "SetDeviceSkillIndex" => super::resource::Handler, SetConduitSkillGroup, Immediate, destination, super::resource::supports_conduit_skill_group;
    [100034] "StopDeviceSkill" => super::resource::Handler, StopConduitSkill, Immediate, destination, arguments::none;
    [60231] "RaspberryAddCount" => super::resource::Handler, RaspberryAddCount, Immediate, destination, super::resource::supports_raspberry_add_count;
    [60233] "RaspberryBigSkill" => super::resource::Handler, RaspberryBigSkill, Immediate, destination, super::resource::supports_raspberry_big_skill;
    [60189] "AddEnergyToCard" => super::card::Handler, AddEnergyToCard, Immediate, destination, super::card::supports_basic_card_energy;
    [60041] "Enchant" => super::card::Handler, EnchantHand, Immediate, once_destination, super::card::supports_enchant_hand;
    [50023] "ChangeToTempCards" => super::card::Handler, ChangeHandToTemporary, Immediate, once_destination, super::card::supports_mark_hand_temporary;
    [60075] "AroundChangeRank" => super::card::Handler, AroundChangeRank, Immediate, queue_preparation, super::card::supports_around_change_rank;
    [50011] "CardLevelChange" => super::card::Handler, CardLevelChange, Immediate, destination, super::card::supports_card_level_change;
    [50034] "ConsumePowerUpgradeSkillCard" => super::card::Handler, ConsumePowerUpgradeSkillCard, Immediate, destination, super::card::supports_power_card_upgrade;
    [60002] "AddUniversalCard" => super::card::Handler, AddUniversalCard, Immediate, destination;
    [60012] "RedealCardKeepStar2" => super::card::Handler, RedealCardKeepStar2, Immediate, destination;
    [60070] "AddUseSkillCard" => super::card::Handler, AddQueuedSkillCard, Immediate, destination, super::card::supports_queued_skill_card;
    [100018] "ConsumeBuffCreateTempCardOrder" => super::precast::Handler, ConsumeBuffCreatePrecast, Immediate, destination, super::precast::supports_arguments;
    [50036] "ConsumePowerDirectUseSkill" => super::use_skill::Handler, ConsumePowerDirectUseSkill, Immediate, destination, super::use_skill::supports_consume_power_direct_skill;
    [60188] "ConsumePowerUseSkill" => super::use_skill::Handler, ConsumePowerUseSkill, Immediate, destination, super::use_skill::supports_consume_power_skill;
    [60121] "ConsumeBuffUseSkill" => super::use_skill::Handler, ConsumeBuffUseSkill, Immediate, once_destination, super::use_skill::supports_consume_buff_use_skill;
    [60311] "ConsumeBuffUseSkill3" => super::use_skill::Handler, ConsumeBuffUseSkill3, Immediate, once_destination, super::use_skill::supports_consume_buff_use_skill3;
    [100007] "EzioReuse" => super::use_skill::Handler, ConsumeTargetBuffUseSkill, Immediate, destination, super::use_skill::supports_consume_target_buff_use_skill;
    [50008] "DirectUseSkill" => super::use_skill::Handler, DirectUseSkill, Immediate, unique_skill_destination, arguments::at_least_one;
    [60053] "DirectUseSkill2" => super::use_skill::Handler, DirectUseSkill2, Immediate, destination;
    [60014] "DirectUseSkillPrev" => super::use_skill::Handler, DirectUseSkillPrev, Immediate, destination, arguments::none;
    [50039] "DirectUseSkillCard" => super::use_skill::Handler, DirectUseSkillCard, Immediate, plain, super::use_skill::supports_direct_skill_card;
    [50012] "DirectUseSkillNoAct" => super::use_skill::Handler, DirectUseSkillNoAct, Immediate, once_destination, super::use_skill::supports_direct_no_action_skill;
    [50038] "DirectUseSkillNoAct2" => super::use_skill::Handler, DirectUseSkillNoAct2, Immediate, destination;
    [60223] "DirectUseSkillNotExtra" => super::use_skill::Handler, DirectUseSkillNotExtra, Immediate, plain;
    [60225] "RandomUseSkill" => super::use_skill::Handler, RandomUseSkill, Immediate, destination, super::use_skill::supports_random_skill;
    [60175] "DirectUseBigSkill" => super::use_skill::Handler, DirectUseBigSkill, Immediate, parent_destination;
    [50010] "DirectUseGroupAndStarSkill" => super::use_skill::Handler, DirectUseGroupAndStarSkill, Immediate, destination, super::use_skill::supports_group_and_star_skill;
    [50015] "UseExtraSkill" => super::use_skill::Handler, UseExtraSkill, Immediate, plain;
    [60242] "CrystalReuse" => super::use_skill::Handler, CrystalReuse, Immediate, destination;
    [60222] "ConsumeCardAddBuff" => super::buff::Handler, ConsumeCardAddBuff, Immediate, destination;
    [60112] "AddTargetBuffByPoison" => super::buff::Handler, AddTargetBuffByPoison, AfterDamage, destination;
    [60142] "ConsumePowerAddBuff" => super::buff::Handler, ConsumePowerAddBuff, Immediate, destination, super::buff::supports_consume_power_add_buff;
    [60150] "ConsumePowerAddMultiBuff1" => super::buff::Handler, ConsumePowerAddMultiBuff1, Immediate, destination, super::buff::supports_consume_power_add_multi_buff;
    [1] "AddBuff" => super::buff::Handler, AddBuff, AfterDamage, aggregated_destination, arguments::at_least_one;
    [2] "AddBuffPowerUse" => super::buff::Handler, AddBuffPowerUse, AfterDamage, aggregated_destination, arguments::at_least_one;
    [1210001] "AddBuff" => super::buff::Handler, AddBuff, AfterDamage, aggregated_destination;
    [1210002] "AddBuff" => super::buff::Handler, AddBuff, AfterDamage, aggregated_destination;
    [20005] "AddBuffRound" => super::buff::Handler, AddBuffRound, AfterDamage, aggregated_destination, super::buff::supports_duration_change;
    [20017] "AddBuffRound2" => super::buff::Handler, AddBuffRound2, AfterDamage, aggregated_destination;
    [20021] "AddBuffRanId" => super::buff::Handler, AddBuffRanId, AfterDamage, destination, super::buff::supports_random_pool;
    [100006] "AddBuffByHeroId" => super::buff::Handler, AddBuffByHeroId, AfterDamage, destination;
    [60029] "RemoveBuffToAddBuff" => super::buff::Handler, RemoveBuffToAddBuff, AfterDamage, destination, arguments::exactly_two;
    [60145] "AddBuffDuration" => super::buff::Handler, AddBuffDuration, Immediate, destination, arguments::exactly_two;
    [50014] "ConsumeBuffByTypeId" => super::buff::Handler, ConsumeBuffByTypeId, AfterDamage, destination, arguments::exactly_two;
    [50016] "ConsumeBuffByTypeId2" => super::buff::Handler, ConsumeBuffByTypeId2, AfterDamage, destination, arguments::exactly_two;
    [60260] "ConsumeBuffLayerAndOtherAddBuff" => super::buff::Handler, ConsumeBuffLayerAndOtherAddBuff, AfterDamage, destination, arguments::exactly_four;
    [30003] "Disperse1" => super::buff::Handler, Disperse1, Immediate, destination, super::buff::supports_status_dispel;
    [30008] "Disperse1" => super::buff::Handler, Disperse1, Immediate, destination;
    [30004] "Disperse2" => super::buff::Handler, Disperse2, Immediate, destination, super::buff::supports_exact_buff_dispel;
    [30009] "Disperse2" => super::buff::Handler, Disperse2, Immediate, destination, super::buff::supports_exact_buff_dispel;
    [60060] "DisperseExclude" => super::buff::Handler, DisperseExclude, Immediate, destination, super::buff::supports_excluded_dispel;
    [60010] "DisperseForce2" => super::buff::Handler, DisperseForce2, Immediate, destination, super::buff::supports_disperse_force;
    [20003] "Purify1" => super::buff::Handler, Purify1, Immediate, destination, super::buff::supports_dispel;
    [20020] "PurifyX" => super::buff::Handler, PurifyX, Immediate, destination, super::buff::supports_dispel;
    [60064] "PurifyX" => super::buff::Handler, PurifyX, Immediate, destination, super::buff::supports_dispel;
    [60085] "DistributeBuff" => super::buff::Handler, DistributeBuff, Immediate, destination, super::buff::supports_distribute;
    [60117] "SelfRandomCopyBuffs" => super::buff::Handler, SelfRandomCopyBuffs, Immediate, destination, super::buff::supports_status_copy;
    [60241] "BuffSortByHp" => super::buff::Handler, BuffSortByHp, Immediate, destination, arguments::at_least_one;
    [60248] "BuffSpread" => super::buff::Handler, BuffSpread, AfterDamage, destination, arguments::exactly_two;
    [50032] "ReplaceBuff" => super::buff::Handler, ReplaceBuff, Immediate, destination, arguments::exactly_four;
    [60176] "ReplaceBuff2" => super::buff::Handler, ReplaceBuff2, Immediate, destination;
    [50035] "AddBuffBasedOnEnemyBurnUseCount" => super::buff::Handler, AddBuffBasedOnEnemyBurnUseCount, Immediate, destination, arguments::exactly_two;
    [60059] "AddBurnBySkillAddBurnCount" => super::buff::Handler, AddBuffBySkillBuffAdditions, Immediate, destination, arguments::at_least_one;
    [60068] "AddBuffByBuffLayer" => super::buff::Handler, AddBuffByBuffLayer, Immediate, destination, arguments::exactly_three;
    [60124] "AddBuffByBuffLayerRange" => super::buff::Handler, AddBuffByBuffLayerRange, Immediate, destination, super::buff::supports_layer_range;
    [60205] "AddBuffAndAddSpecialCount" => super::special_count::Handler, AddBuffAndAddSpecialCount, Immediate, transfer, super::special_count::supports_add_buff_and_count;
    [60204] "AddBuffSpecialCount" => super::special_count::Handler, AddBuffSpecialCount, Immediate, transfer, super::special_count::supports_add_count;
    [60202] "AddSkillRateBySpecialCount" => super::special_count::Handler, AddSkillRateBySpecialCount, Immediate, modifier, super::special_count::supports_rate;
    [60190] "BloodPoolMaxChange" => super::gauge::Handler, BloodPoolMaxChange, Immediate, parent_destination, super::gauge::supports_shared_pool_mutation;
    [60191] "BloodPoolValueChange" => super::gauge::Handler, BloodPoolValueChange, AfterDamage, parent_destination, super::gauge::supports_shared_pool_mutation;
    [60210] "ConsumeBloodAddBuff" => super::gauge::Handler, ConsumeBloodAddBuff, Immediate, destination, @route(ConditionRouteOverride::Setup { key: DefinitionKey::new(57104, "NoBuffId"), stage: SetupStage::RoundStart, priority: 3 }), super::gauge::supports_consume_blood_add_buff;
    [60211] "ConsumeBloodAddBuff2" => super::gauge::Handler, ConsumeBloodAddBuff2, Immediate, destination, @route(ConditionRouteOverride::Setup { key: DefinitionKey::new(57104, "NoBuffId"), stage: SetupStage::RoundStart, priority: 3 }), super::gauge::supports_consume_blood_add_buff;
    [50019] "AddMagicCircle" => super::magic_circle::Handler, AddMagicCircle, Immediate, destination;
    [60076] "MagicCircleAttr" => super::magic_circle::Handler, MagicCircleAttr, Immediate, plain;
    [60195] "ElectricTransform" => super::electric::Handler, ElectricTransform, Immediate, destination, super::electric::supports;
    [100000] "EzioProps" => super::synchronization::Handler, EzioProps, Immediate, destination;
    [100001] "EzioBigSkillTyp1" => super::synchronization::Handler, EzioBigSkillType1, AfterDamage, destination;
    [100002] "EzioBigSkillTyp2" => super::synchronization::Handler, EzioBigSkillType2, AfterDamage, destination;
    [100003] "EzioBigSkillEnd" => super::synchronization::Handler, EzioBigSkillEnd, AfterDamage, destination;
    [100022] "EzioBigSkillCheckTimes" => super::synchronization::Handler, EzioBigSkillCheckTimes, AfterHit, destination, @route(ConditionRouteOverride::Trigger { key: DefinitionKey::new(214, "None"), event: EventKind::SkillAction, phase: Some(SkillPhase::AfterHit) });
    [100012] "EzioBigSkillWeapon2" => super::ultimate_kind::Handler, UltimateExtraAction, Immediate, destination;
    [40009] "AddSummoned" => super::summon::Handler, AddSummoned, Immediate, destination;
    [40010] "ChangeSummonedLevel" => super::summon::Handler, ChangeSummonedLevel, Immediate, destination;
    [40011] "AddSummonedLevel" => super::summon::Handler, AddSummonedLevel, Immediate, destination;
    [40012] "RemoveSummoned" => super::summon::Handler, RemoveSummoned, Immediate, destination;
    [60008] "Summon" => super::summon::Handler, Summon, Immediate, destination, super::summon::supports_combatant;
    [60056] "SummonSp2" => super::summon::Handler, SummonSp2, AfterDamage, destination;
    [60015] "Kill" => super::kill::Handler, Kill, Immediate, destination, super::kill::supports;
    [60018] "Kill" => super::kill::Handler, LethalHpLoss, Immediate, destination, super::kill::supports;
    [60019] "KillTargets" => super::kill::Handler, KillTargets, AfterDamage, destination;
    [40006] "MonsterChange" => super::monster_change::Handler, MonsterChange, Immediate, destination, super::monster_change::supports;
    [60074] "CatapultBuff" => super::poison::Handler, CatapultBuff, AfterDamage, destination;
    [60110] "PoisonConvertToTargetBuff" => super::poison::Handler, PoisonConvertToTargetBuff, AfterDamage, destination;
    [60111] "ConsumePoisonSettleDeadlyPoison" => super::poison::Handler, ConsumePoisonSettleDeadlyPoison, AfterDamage, destination;
    [100005] "Assassinate" => super::general::AssassinateHandler, Assassinate, Immediate, destination, arguments::none;
    [60037] "NotifyUpgradeHero" => super::general::Handler, NotifyUpgradeHero, Immediate, destination;
    [60198] "ClientEffect" => super::general::Handler, ClientEffect, Immediate, destination, arguments::at_least_one;
    [60268] "ChangeScene" => super::scene::Handler, ChangeScene, Immediate, destination;
    [60058] "CareerRatioFix" => super::career::Handler, CareerRatioFix, Immediate, modifier;
    [100036] "SkillChangeAttackCareer" => super::career::Handler, ChangeAttackCareer, Immediate, modifier;
    [40003] "AddAct" => super::action_point::Handler, AddAct, Immediate, round_modifier_only;
    [40007] "AddActAndCardLimit" => super::card_limit::Handler, AddActAndCardLimit, AfterDamage, round_modifier_with_output;
    [60221] "IgnoreSkillConfigDamageRate" => super::general::DamageRateMarkerHandler, IgnoreSkillConfigDamageRate, Immediate, destination, arguments::none;
    [100017] "IgnoreSkillConfigDamageRate" => super::general::DamageRateMarkerHandler, IgnoreSkillConfigDamageRate, Immediate, destination, arguments::none;
    [60036] "ConsumeBuffChangeTargets" => super::skill_modifier::Handler, ConsumeBuffChangeTargets, Immediate, destination;
    [60034] "ConsumeBuffUpSkillDamageRate" => super::skill_modifier::Handler, ConsumeBuffUpSkillDamageRate, Immediate, destination;
    [60035] "ConsumeBuffAttrFix" => super::skill_modifier::Handler, ConsumeBuffAttrFix, Immediate, destination;
    [100019] "ConsumeBuffFixMixedRate" => super::skill_modifier::Handler, ConsumeBuffFixMixedRate, Immediate, destination, super::skill_modifier::supports_mixed_rate;
    [60206] "CreateAdditionalDamageAddBuff" => super::additional_damage::Handler, CreateAdditionalDamageAddBuff, Immediate, destination;
    [60209] "NuoDiKaDamage" => super::nuo_di_ka::Handler, NuoDiKaDamage, Immediate, destination;
    [60082] "Redirect" => super::damage_target::Handler, ConfiguredDamageTarget, Immediate, destination;
    [10004] "AttrFix" => super::rate::Handler, AttrFix, Immediate, modifier, super::rate::supports_attr_fix;
    [10001] "SkillRateUp" => super::rate::Handler, SkillRateUp, Immediate, modifier, arguments::at_least_one;
    [10002] "SkillRateUp1" => super::rate::Handler, SkillRateUp1, Immediate, modifier, super::rate::supports_status_skill_rate;
    [10003] "SkillRateUp2" => super::rate::Handler, SkillRateUp2, Immediate, modifier, super::rate::supports_status_skill_rate;
    [60067] "SkillRateUpCardLevel" => super::rate::Handler, SkillRateUpCardLevel, Immediate, modifier, super::rate::supports_card_rank_skill_rate;
    [60234] "AddSkillRateByTargetCount" => super::rate::Handler, AddSkillRateByTargetCount, Immediate, modifier, super::rate::supports_target_count_rate;
    [60182] "SkillRateUpBySelfBuffType" => super::rate::Handler, SkillRateUpBySelfBuffType, Immediate, modifier, super::rate::supports_self_buff_type_rate;
    [60174] "ConsumeExPointAddAttr" => super::rate::Handler, ConsumeExPointAddAttr, Immediate, modifier;
    [60255] "HeatScaleAddSkillRate" => super::rate::Handler, HeatScaleAddSkillRate, Immediate, modifier, super::rate::supports_heat_scale_rate;
    [100030] "TwinsUpByCounter" => super::rate::Handler, ConduitRateByConsumedPower, Immediate, modifier, super::rate::supports_conduit_rate;
    [100031] "TwinsPowerUp" => super::rate::Handler, ConduitPowerUp, Immediate, modifier, super::rate::supports_conduit_power_up;
    [60243] "CrystalAddSkillRate" => super::rate::Handler, CrystalAddSkillRate, Immediate, destination;
    [60244] "CrystalAddCardRank" => super::rate::Handler, CrystalAddCardRank, Immediate, destination;
    [60086] "BulletCritRateAlter" => super::rate::Handler, BulletCritRateAlter, Immediate, modifier, super::rate::supports_bullet_crit_rate;
    [40001] "CritRateAlter" => super::rate::Handler, CritRateAlter, Immediate, modifier, super::rate::supports_crit_rate_alter;
    [100023] "CritRateAlter2" => super::rate::Handler, CritRateAlter2, Immediate, modifier, super::rate::supports_crit_rate_alter;
    [60228] "CritRateAlter2" => super::rate::Handler, CritRateAlter2, Immediate, modifier, super::rate::supports_crit_rate_alter;
    [60069] "MustCrit" => super::skill_modifier::Handler, MustCrit, Immediate, modifier, arguments::none;
    [60054] "IgnoreBeatBack" => super::skill_modifier::Handler, IgnoreBeatBack, Immediate, modifier, arguments::none;
    [10006] "Damage" => crate::engine::damage::handler::Handler, Damage, Immediate, destination, crate::engine::damage::handler::supports_attribute_damage;
    [10008] "Damage2" => crate::engine::damage::handler::Handler, Damage2, Immediate, plain;
    [30014] "OriginDamage" => crate::engine::damage::handler::Handler, OriginDamage, AfterDamage, destination, crate::engine::damage::handler::supports_origin_damage;
    [30015] "OriginDamageCanCrit" => crate::engine::damage::handler::Handler, OriginDamageCanCrit, AfterDamage, destination, crate::engine::damage::handler::supports_origin_damage;
    [60146] "OriginDamageByTeamAttr" => crate::engine::damage::handler::Handler, OriginDamageByTeamAttr, AfterDamage, plain, crate::engine::damage::handler::supports_team_attr_damage;
    [60127] "OriginDamageByAttrAndBuffGroupSize" => crate::engine::damage::handler::Handler, OriginDamageByAttrAndBuffGroupSize, AfterDamage, plain;
    [60282] "ButterflyDamage" => crate::engine::damage::handler::Handler, ButterflyDamage, AfterDamage, destination, crate::engine::damage::handler::supports_butterfly_damage;
    [20001] "Heal" => crate::engine::damage::handler::Handler, Heal, AfterDamage, destination, crate::engine::damage::handler::supports_heal;
    [90001] "Heal" => crate::engine::damage::handler::Handler, Heal, AfterDamage, destination, crate::engine::damage::handler::supports_heal;
    [20016] "HealCantCrit" => crate::engine::damage::handler::Handler, HealCantCrit, AfterDamage, destination, crate::engine::damage::handler::supports_attr_heal;
    [60232] "HealByTwoAttr" => crate::engine::damage::handler::Handler, HealByTwoAttr, AfterDamage, destination, crate::engine::damage::handler::supports_two_attr_heal;
    [20010] "Bloodlust" => crate::engine::damage::handler::Handler, Bloodlust, AfterDamage, destination, crate::engine::damage::handler::supports_bloodlust;
    [30005] "LostLife" => crate::engine::damage::handler::Handler, LostLife, Immediate, destination, crate::engine::damage::handler::supports_lost_life;
    [30006] "LostLife" => crate::engine::damage::handler::Handler, LostLife, Immediate, destination, crate::engine::damage::handler::supports_lost_life;
    [30018] "LostLife" => crate::engine::damage::handler::Handler, LostLife, Immediate, destination, crate::engine::damage::handler::supports_lost_life;
    [60288] "LostLife" => crate::engine::damage::handler::Handler, LostLife, Immediate, destination, crate::engine::damage::handler::supports_lost_life;
    [60310] "LostLife" => crate::engine::damage::handler::Handler, ToughnessOverflowDamage, Immediate, destination, crate::engine::damage::handler::supports_lost_life;
    [60212] "LostAllLifeByAttr" => crate::engine::damage::handler::Handler, LostAllLifeByAttr, Immediate, destination, crate::engine::damage::handler::supports_lost_all_life_by_attr;
    [60216] "DamageRealLostLife" => crate::engine::damage::handler::Handler, DamageRealLostLife, Immediate, destination, crate::engine::damage::handler::supports_damage_real_lost_life;
    [60038] "OriginDamageFromInjuryBankBuff" => super::injury_bank::Handler, OriginDamageFromInjuryBankBuff, AfterDamage, destination;
    [60052] "OriginDamageFromInjuryBankBuff" => super::injury_bank::Handler, OriginDamageFromInjuryBankBuff, AfterDamage, destination;
    [60039] "RealDamageSelfAndAddBuffToTarget" => super::injury_bank::Handler, RealDamageSelfAndAddBuffToTarget, Immediate, destination;
    [60040] "ClearInjuryBankBuffOriginDamage" => super::injury_bank::Handler, ClearInjuryBankBuffOriginDamage, AfterDamage, destination;
    [20009] "Detonate2" => super::detonate::Handler, Detonate2, AfterDamage, destination;
    [60183] "SupplyShield2" => super::shield::ChildUidHandler, SupplyShield2, Immediate, destination;
    [60259] "SupplyShield2" => super::shield::Handler, SupplyShield2, Immediate, destination;
    [60290] "SupplyTeamShareShield" => super::shield::Handler, SupplyTeamShareShield, Immediate, setup_parent_destination;
    [60133] "ShellAssign" => super::shell::Handler, ShellAssign, AfterDamage, destination, super::shell::supports_assign;
    [60134] "ShellRecycle" => super::shell::Handler, ShellRecycle, AfterDamage, destination, super::shell::supports_recycle;
    [60135] "ShellUseSkill" => super::shell::Handler, ShellUseSkill, Immediate, destination, super::shell::supports_use_skill;
    [60245] "CrystalAddCard" => super::crystal_card::Handler, CrystalAddCard, Immediate, destination, super::crystal_card::supports_arguments;
    [60065] "AttrFixByBurnLayerAndExtraBurnHurt" => super::attr_fix_by_burn_layer::Handler, AttrFixByBurnLayerAndExtraBurnHurt, Immediate, modifier;
    [60033] "AttrFixByLoseHp" => super::attr_fix_by_lost_hp::Handler, AttrFixByLoseHp, Immediate, modifier;
    [60246] "HeatScaleUseSkillAddCount" => crate::engine::mechanic::heat_scale::Handler, HeatScaleUseSkillAddCount, Immediate, destination, arguments::at_least_one;
    [60247] "AddCardRankNext" => crate::engine::mechanic::heat_scale::Handler, AddCardRankNext, Immediate, queue_preparation, arguments::exactly_two;
    [60281] "AddCardRankByEffectTag" => super::card::Handler, AddCardRankByEffectTag, Immediate, queue_preparation, super::card::supports_rank_by_effect_tag;
    [60283] "BufferflyRecordSkill" => super::card::Handler, BufferflyRecordSkill, Immediate, destination, arguments::none;
    [60287] "ToughnessRecover" => super::toughness::Handler, ToughnessRecover, Immediate, destination;
    [60254] "AddHeatScaleFromBuff" => crate::engine::mechanic::heat_scale::Handler, AddHeatScaleFromBuff, Immediate, destination, arguments::none;
}

/// Exact behavior support and dispatch boundary.
/// Missing definitions remain unsupported; callers must not infer a family or invoke a handler directly.
pub fn find(behavior: &ParsedBehavior) -> Option<&'static BehaviorDefinition> {
    find_key(behavior.spec.key.opcode, &behavior.spec.key.type_name)
        .filter(|definition| definition.kind == behavior.spec.kind)
}

pub fn find_key(opcode: i32, type_name: &str) -> Option<&'static BehaviorDefinition> {
    definitions().find(|definition| definition.key.matches(opcode, type_name))
}

pub fn definitions() -> impl Iterator<Item = &'static BehaviorDefinition> {
    DEFINITIONS.iter()
}

#[cfg(test)]
mod tests;
