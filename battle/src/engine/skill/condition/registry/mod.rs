use super::{
    act_order, active_skill, battle_tag, buff, card, career, conduit, entity_count, extra, hp,
    injury, lifecycle, magic_circle, none,
    parse::{self, ParsedCondition, ParsedConditionKind},
    resource, target_identity, trigger,
};
use crate::engine::{
    event::{
        kind::EventKind,
        subscription::{PublicationPhase, ReactionTiming},
    },
    skill::{
        action::SkillPhase,
        rule::{DefinitionKey, SetupStage},
    },
};

pub type Parser = fn(i32, &str, &[String]) -> Option<ParsedConditionKind>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionRole {
    Predicate,
    Trigger {
        event: EventKind,
        phase: Option<SkillPhase>,
    },
    Setup {
        stage: SetupStage,
        priority: i32,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SetupFrameScope {
    Side,
    #[default]
    Entity,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ConsequencePolicy {
    #[default]
    Default,
    ChildBuffGrant,
    NormalBuffGrant,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BehaviorOwnership {
    #[default]
    Skill,
    MatchingBuffAct,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BehaviorTargetSource {
    #[default]
    Resolved,
    ActiveSkillTargets,
    HitTargets,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReactionFrameTarget {
    #[default]
    Counterparty,
    Owner,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReactionFrameScope {
    #[default]
    Subscriber,
    Causing,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SkillActionObserver {
    #[default]
    Actor,
    AttackTarget,
    Team,
    OpposingTeam,
    AllyOfAttackedTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttackModifierSide {
    IncomingTarget,
}

pub struct ConditionDefinition {
    pub key: DefinitionKey,
    pub parse: Parser,
    pub role: ConditionRole,
    pub dependencies: &'static [EventKind],
    pub publication: PublicationPhase,
    pub reaction_timing: ReactionTiming,
    pub consequence: ConsequencePolicy,
    pub behavior_ownership: BehaviorOwnership,
    pub filters_behavior_targets: bool,
    pub behavior_target_source: BehaviorTargetSource,
    pub reaction_frame_target: ReactionFrameTarget,
    pub reaction_frame_scope: ReactionFrameScope,
    pub skill_action_observer: SkillActionObserver,
    pub attack_modifier_side: Option<AttackModifierSide>,
    pub companion_setup: &'static [(SetupStage, i32)],
    pub reactivation_events: &'static [EventKind],
    pub setup_frame_scope: SetupFrameScope,
}

#[derive(Debug, Clone, Copy)]
pub struct ConditionMetadata {
    pub role: ConditionRole,
    pub dependencies: &'static [EventKind],
    pub publication: PublicationPhase,
    pub reaction_timing: ReactionTiming,
    pub consequence: ConsequencePolicy,
    pub behavior_ownership: BehaviorOwnership,
    pub filters_behavior_targets: bool,
    pub behavior_target_source: BehaviorTargetSource,
    pub reaction_frame_target: ReactionFrameTarget,
    pub reaction_frame_scope: ReactionFrameScope,
    pub skill_action_observer: SkillActionObserver,
    pub attack_modifier_side: Option<AttackModifierSide>,
    pub companion_setup: &'static [(SetupStage, i32)],
    pub reactivation_events: &'static [EventKind],
    pub setup_frame_scope: SetupFrameScope,
}

pub const fn definition(
    opcode: i32,
    type_name: &'static str,
    parse: Parser,
    metadata: ConditionMetadata,
) -> ConditionDefinition {
    ConditionDefinition {
        key: DefinitionKey::new(opcode, type_name),
        parse,
        role: metadata.role,
        dependencies: metadata.dependencies,
        publication: metadata.publication,
        reaction_timing: metadata.reaction_timing,
        consequence: metadata.consequence,
        behavior_ownership: metadata.behavior_ownership,
        filters_behavior_targets: metadata.filters_behavior_targets,
        behavior_target_source: metadata.behavior_target_source,
        reaction_frame_target: metadata.reaction_frame_target,
        reaction_frame_scope: metadata.reaction_frame_scope,
        skill_action_observer: metadata.skill_action_observer,
        attack_modifier_side: metadata.attack_modifier_side,
        companion_setup: metadata.companion_setup,
        reactivation_events: metadata.reactivation_events,
        setup_frame_scope: metadata.setup_frame_scope,
    }
}

pub const fn predicate(dependencies: &'static [EventKind]) -> ConditionMetadata {
    ConditionMetadata {
        role: ConditionRole::Predicate,
        dependencies,
        publication: PublicationPhase::AfterPublish,
        reaction_timing: ReactionTiming::Immediate,
        consequence: ConsequencePolicy::Default,
        behavior_ownership: BehaviorOwnership::Skill,
        filters_behavior_targets: false,
        behavior_target_source: BehaviorTargetSource::Resolved,
        reaction_frame_target: ReactionFrameTarget::Counterparty,
        reaction_frame_scope: ReactionFrameScope::Subscriber,
        skill_action_observer: SkillActionObserver::Actor,
        attack_modifier_side: None,
        companion_setup: &[],
        reactivation_events: &[],
        setup_frame_scope: SetupFrameScope::Entity,
    }
}

pub const fn event_trigger(event: EventKind, phase: Option<SkillPhase>) -> ConditionMetadata {
    ConditionMetadata {
        role: ConditionRole::Trigger { event, phase },
        dependencies: &[],
        publication: PublicationPhase::AfterPublish,
        reaction_timing: ReactionTiming::Immediate,
        consequence: ConsequencePolicy::Default,
        behavior_ownership: BehaviorOwnership::Skill,
        filters_behavior_targets: false,
        behavior_target_source: BehaviorTargetSource::Resolved,
        reaction_frame_target: ReactionFrameTarget::Counterparty,
        reaction_frame_scope: ReactionFrameScope::Subscriber,
        skill_action_observer: SkillActionObserver::Actor,
        attack_modifier_side: None,
        companion_setup: &[],
        reactivation_events: &[],
        setup_frame_scope: SetupFrameScope::Entity,
    }
}

pub const fn setup_route(
    stage: SetupStage,
    priority: i32,
    dependencies: &'static [EventKind],
) -> ConditionMetadata {
    ConditionMetadata {
        role: ConditionRole::Setup { stage, priority },
        dependencies,
        publication: PublicationPhase::AfterPublish,
        reaction_timing: ReactionTiming::Immediate,
        consequence: ConsequencePolicy::Default,
        behavior_ownership: BehaviorOwnership::Skill,
        filters_behavior_targets: false,
        behavior_target_source: BehaviorTargetSource::Resolved,
        reaction_frame_target: ReactionFrameTarget::Counterparty,
        reaction_frame_scope: ReactionFrameScope::Subscriber,
        skill_action_observer: SkillActionObserver::Actor,
        attack_modifier_side: None,
        companion_setup: &[],
        reactivation_events: &[],
        setup_frame_scope: SetupFrameScope::Entity,
    }
}

pub const fn before_publish(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.publication = PublicationPhase::BeforePublish;
    metadata
}

pub const fn after_skill(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.reaction_timing = ReactionTiming::AfterSkill;
    metadata
}

pub const fn child_buff_grant(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.consequence = ConsequencePolicy::ChildBuffGrant;
    metadata
}

pub const fn normal_buff_grant(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.consequence = ConsequencePolicy::NormalBuffGrant;
    metadata
}

pub const fn matching_buff_act_owns_behavior(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.behavior_ownership = BehaviorOwnership::MatchingBuffAct;
    metadata
}

pub const fn filters_behavior_targets(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.filters_behavior_targets = true;
    metadata
}

pub const fn uses_active_skill_targets(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.behavior_target_source = BehaviorTargetSource::ActiveSkillTargets;
    metadata
}

pub const fn uses_hit_targets(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.behavior_target_source = BehaviorTargetSource::HitTargets;
    metadata
}

pub const fn reaction_targets_owner(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.reaction_frame_target = ReactionFrameTarget::Owner;
    metadata
}

pub const fn in_causing_frame(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.reaction_frame_scope = ReactionFrameScope::Causing;
    metadata
}

pub const fn ally_of_attacked_target_observes(
    mut metadata: ConditionMetadata,
) -> ConditionMetadata {
    metadata.skill_action_observer = SkillActionObserver::AllyOfAttackedTarget;
    metadata
}

pub const fn attack_target_observes(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.skill_action_observer = SkillActionObserver::AttackTarget;
    metadata
}

pub const fn team_observes(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.skill_action_observer = SkillActionObserver::Team;
    metadata
}

pub const fn opposing_team_observes(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.skill_action_observer = SkillActionObserver::OpposingTeam;
    metadata
}

pub const fn incoming_attack_modifier(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.attack_modifier_side = Some(AttackModifierSide::IncomingTarget);
    metadata
}

pub const fn companion_setup(
    mut metadata: ConditionMetadata,
    routes: &'static [(SetupStage, i32)],
) -> ConditionMetadata {
    metadata.companion_setup = routes;
    metadata
}

pub const fn reactivates_on(
    mut metadata: ConditionMetadata,
    events: &'static [EventKind],
) -> ConditionMetadata {
    metadata.reactivation_events = events;
    metadata
}

pub const fn setup_in_side_frame(mut metadata: ConditionMetadata) -> ConditionMetadata {
    metadata.setup_frame_scope = SetupFrameScope::Side;
    metadata
}

macro_rules! condition_definitions {
    ($([$($opcode:expr),+ $(,)?] $type_name:literal => $parse:path, $role:expr);+ $(;)?) => {
        pub const DEFINITIONS: &[ConditionDefinition] =
            &[$($(definition($opcode, $type_name, $parse, $role)),+),+];
    };
}

condition_definitions! {
    [0] "None" => none::always, predicate(&[]);
    [5] "EnterFight" => lifecycle::enter_fight, reactivates_on(setup_route(SetupStage::EnterFight, 0, &[]), &[EventKind::EntityTransformed]);
    [55, 1050, 655036, 655038] "None" => none::enter_battle, companion_setup(event_trigger(EventKind::EntityEntered, None), &[(SetupStage::EnterBattleStatic, 0)]);
    [6] "None" => none::unconditional, setup_route(SetupStage::Unconditional, 0, &[]);
    [5021] "EnterFight" => lifecycle::battle_start, setup_route(SetupStage::BattleStart, 0, &[]);
    [100] "None" => none::round_start, setup_route(SetupStage::RoundStart, -1, &[]);
    [101] "None" => none::round_start, setup_route(SetupStage::RoundStartCondition, 101, &[]);
    [102] "None" => none::round_start, setup_route(SetupStage::RoundStartCondition, 102, &[]);
    [103] "None" => none::round_start, setup_route(SetupStage::RoundStart, 1, &[]);
    [104] "None" => none::round_start, setup_route(SetupStage::RoundStartLate, 0, &[]);
    [45100] "HeroRoundInterval" => lifecycle::period_then_start, setup_route(SetupStage::RoundStart, -1, &[]);
    [45101] "HeroRoundInterval" => lifecycle::period_then_start, setup_route(SetupStage::RoundStartCondition, 101, &[]);
    [727100] "RoundAfter" => lifecycle::after_round, setup_route(SetupStage::RoundStartCondition, 100, &[]);
    [45102] "HeroRoundInterval" => lifecycle::round_interval, setup_route(SetupStage::RoundTransitionStart, 0, &[]);
    [45104] "HeroRoundInterval" => lifecycle::period_then_start, setup_route(SetupStage::RoundTransitionStart, 1, &[]);
    [45106] "HeroRoundInterval" => lifecycle::period_then_start, setup_route(SetupStage::CardSetup, 0, &[]);
    [45302] "HeroRoundInterval" => lifecycle::period_then_start, event_trigger(EventKind::RoundEnd, None);
    [45303] "HeroRoundInterval" => lifecycle::period_then_start, reaction_targets_owner(event_trigger(EventKind::RoundEndEntitySettlement, None));
    [10411] "None" => none::round_start, setup_route(SetupStage::RoundStart, 3, &[]);
    [105] "None" => none::after_round_start, setup_route(SetupStage::AfterRoundStart, 0, &[]);
    [106] "None" => none::card_setup, setup_route(SetupStage::CardSetup, 0, &[]);
    [552106] "Random" => parse::random, setup_route(SetupStage::CardSetup, 0, &[]);
    [107] "None" => none::before_ap_resolve, companion_setup(event_trigger(EventKind::ActionQueueCommitted, None), &[(SetupStage::GeneratedCard, 0)]);
    [203] "None" => none::skill_action_start, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [201] "None" => none::skill_action_start, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [205] "None" => none::skill_action_start, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [208] "None" => none::skill_action_after_damage, uses_hit_targets(event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage)));
    [210] "None" => none::skill_action_after_hit, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [202, 204, 206, 207, 2011, 2082, 900, 901, 903, 905, 908, 910, 930, 1041] "None" => none::skill_action, event_trigger(EventKind::SkillAction, None);
    [2092] "None" => trigger::parse_guard_broken, reaction_targets_owner(event_trigger(EventKind::ToughnessBroken, None));
    [783101] "IsBroken" => trigger::parse_entity_broken, setup_route(SetupStage::RoundStartCondition, 101, &[]);
    [783102] "IsBroken" => trigger::parse_entity_broken, setup_route(SetupStage::RoundStartCondition, 102, &[]);
    [1061] "None" => none::action_queue_committed, event_trigger(EventKind::ActionQueueCommitted, None);
    [2081] "None" => none::skill_cast, uses_active_skill_targets(event_trigger(EventKind::SkillCast, None));
    [209, 211] "None" => none::attacked, event_trigger(EventKind::BeAttacked, None);
    [212, 52008] "None" => none::ally_action, event_trigger(EventKind::AllyAction, None);
    [214] "None" => none::shell_deploy, event_trigger(EventKind::ShellDeployed, None);
    [215] "None" => none::shell_retrieve, event_trigger(EventKind::ShellRetrieved, None);
    [301] "None" => none::small_round_end, event_trigger(EventKind::SmallRoundEnd, None);
    [302] "None" => none::round_end, event_trigger(EventKind::RoundEnd, None);
    [305, 306] "None" => none::round_end, event_trigger(EventKind::RoundEnd, None);
    [303] "None" => none::round_end_entity_settlement, reaction_targets_owner(event_trigger(EventKind::RoundEndEntitySettlement, None));
    [307] "None" => none::round_end_final_settlement, event_trigger(EventKind::RoundEndFinalSettlement, None);
    [304] "None" => none::round_end_after_settlement, reaction_targets_owner(event_trigger(EventKind::RoundEndAfterSettlement, None));
    [402] "None" => none::skill_after_attack, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [552402] "Random" => parse::random, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [620402] "CurrSkillLevel" => active_skill::rank, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [931] "None" => none::impromptu_resolved, event_trigger(EventKind::ImpromptuResolved, None);
    [19002] "HasBuffId" => buff::buff_present, companion_setup(filters_behavior_targets(predicate(&[])), &[(SetupStage::EnterFight, 0)]);
    [19003] "HasBuffId" => buff::buff_present, filters_behavior_targets(predicate(&[EventKind::BuffAdded, EventKind::BuffChanged]));
    [19004] "HasBuffId" => buff::buff_present, filters_behavior_targets(predicate(&[EventKind::BuffAdded, EventKind::BuffChanged]));
    [19012] "HasBuffId" => buff::buff_present, filters_behavior_targets(predicate(&[]));
    [19100] "HasBuffId" => buff::buff_present, filters_behavior_targets(setup_route(SetupStage::RoundStartCondition, 100, &[]));
    [19101] "HasBuffId" => buff::buff_present, filters_behavior_targets(setup_route(SetupStage::RoundStartCondition, 101, &[]));
    [19102] "HasBuffId" => buff::buff_present, filters_behavior_targets(setup_route(SetupStage::RoundStartCondition, 102, &[]));
    [18201] "HasBuff" => buff::any_status_present, predicate(&[EventKind::BuffChanged]);
    [18202] "HasBuff" => buff::any_status_present, incoming_attack_modifier(event_trigger(EventKind::SkillAction, None));
    [18203] "HasBuff" => buff::first_status_present, predicate(&[EventKind::BuffChanged]);
    [18208] "HasBuff" => buff::any_status_present, filters_behavior_targets(event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage)));
    [18301] "HasBuff" => buff::first_status_present, filters_behavior_targets(setup_route(SetupStage::RoundStartCondition, 101, &[]));
    [18302] "HasBuff" => buff::first_status_present, event_trigger(EventKind::RoundEnd, None);
    [19104] "HasBuffId" => buff::buff_present, filters_behavior_targets(setup_route(SetupStage::BuffSync, 0, &[]));
    [19105] "HasBuffId" => buff::buff_present, filters_behavior_targets(setup_route(SetupStage::AfterRoundStart, 0, &[]));
    [19021, 19201] "HasBuffId" => buff::buff_present, filters_behavior_targets(predicate(&[EventKind::BuffChanged]));
    [19204] "HasBuffId" => buff::buff_present, incoming_attack_modifier(filters_behavior_targets(predicate(&[EventKind::BuffChanged])));
    [19205] "HasBuffId" => buff::exact_buff_present, filters_behavior_targets(predicate(&[]));
    [19203] "HasBuffId" => buff::buff_present, filters_behavior_targets(event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate)));
    [192032] "HasBuffId" => buff::buff_present, filters_behavior_targets(predicate(&[EventKind::BuffChanged]));
    [19208] "HasBuffId" => buff::buff_present_and_consume, filters_behavior_targets(event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage)));
    [19209] "HasBuffId" => buff::buff_present, filters_behavior_targets(predicate(&[EventKind::TargetAttacked]));
    [19210] "HasBuffId" => buff::buff_present, filters_behavior_targets(predicate(&[EventKind::SkillAction]));
    [19213] "HasBuffId" => buff::buff_present, filters_behavior_targets(event_trigger(EventKind::SkillAction, Some(SkillPhase::HitPassives)));
    [192081] "HasBuffId" => buff::buff_present, filters_behavior_targets(predicate(&[]));
    [19103] "HasBuffId" => buff::buff_present, filters_behavior_targets(setup_route(SetupStage::BuffGate, 0, &[]));
    [19212] "HasBuffId" => buff::buff_present, filters_behavior_targets(predicate(&[EventKind::BuffChanged]));
    [19302] "HasBuffId" => buff::buff_present, filters_behavior_targets(event_trigger(EventKind::RoundEnd, None));
    [19304] "HasBuffId" => buff::buff_present, filters_behavior_targets(event_trigger(EventKind::RoundEnd, None));
    [19301] "HasBuffId" => buff::buff_present, filters_behavior_targets(event_trigger(EventKind::SmallRoundEnd, None));
    [19402] "HasBuffId" => buff::buff_present, filters_behavior_targets(predicate(&[]));
    [56301] "NoBuff" => buff::first_status_absent, filters_behavior_targets(event_trigger(EventKind::SmallRoundEnd, None));
    [750101] "PlayerHasBuff" => buff::team_buff_presence, setup_route(SetupStage::RoundStartCondition, 101, &[]);
    [514100] "SelfTeamHasBuffTypeLayerLessThan" => buff::team_buff_type_layer_at_most, setup_route(SetupStage::RoundStartCondition, 100, &[EventKind::BuffChanged]);
    [57208] "NoBuffId" => buff::buff_absent, filters_behavior_targets(event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage)));
    [57204] "NoBuffId" => buff::buff_absent, incoming_attack_modifier(filters_behavior_targets(predicate(&[EventKind::BuffChanged])));
    [57210] "NoBuffId" => buff::buff_absent, filters_behavior_targets(predicate(&[EventKind::SkillAction]));
    [572081] "NoBuffId" => buff::buff_absent, filters_behavior_targets(predicate(&[]));
    [57002] "NoBuffId" => buff::buff_absent, filters_behavior_targets(setup_route(SetupStage::EnterFight, 0, &[]));
    [57012] "NoBuffId" => buff::buff_absent, filters_behavior_targets(predicate(&[]));
    [57100] "NoBuffId" => buff::buff_absent, filters_behavior_targets(predicate(&[EventKind::RoundStart]));
    [57104] "NoBuffId" => buff::buff_absent, filters_behavior_targets(predicate(&[EventKind::RoundStart]));
    [57213] "NoBuffId" => buff::buff_absent, filters_behavior_targets(event_trigger(EventKind::SkillAction, Some(SkillPhase::HitPassives)));
    [57301] "NoBuffId" => buff::buff_absent, filters_behavior_targets(event_trigger(EventKind::SmallRoundEnd, None));
    [57304] "NoBuffId" => buff::buff_absent, filters_behavior_targets(event_trigger(EventKind::RoundEndAfterSettlement, None));
    [539301] "PerSelfTeamTypeType2BuffTypeIdNum" => buff::per_team_status_type_count, event_trigger(EventKind::SmallRoundEnd, None);
    [51201] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [51203] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [51210] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [51212] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, predicate(&[]);
    [51213] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, predicate(&[]);
    [51302] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, event_trigger(EventKind::RoundEnd, None);
    [51303] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, event_trigger(EventKind::RoundEndEntitySettlement, None);
    [535208] "TypeIdBuffCountMoreThan" => buff::buff_type_at_least, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [535201] "TypeIdBuffCountMoreThan" => buff::buff_type_at_least, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [535210] "TypeIdBuffCountMoreThan" => buff::buff_type_at_least, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [535212] "TypeIdBuffCountMoreThan" => buff::any_target_buff_type_at_least, event_trigger(EventKind::AllyAction, None);
    [535203] "TypeIdBuffCountMoreThan" => buff::buff_type_at_least, predicate(&[]);
    [535104] "TypeIdBuffCountMoreThan" => buff::buff_type_at_least, setup_route(SetupStage::RoundStartLate, 0, &[]);
    [535214] "TypeIdBuffCountMoreThan" => buff::buff_type_at_least, event_trigger(EventKind::TargetAttacked, None);
    [535215] "TypeIdBuffCountMoreThan" => buff::buff_type_at_least, event_trigger(EventKind::AllyAction, None);
    [535303] "TypeIdBuffCountMoreThan" => buff::buff_type_at_least, event_trigger(EventKind::RoundEndEntitySettlement, None);
    [535304] "TypeIdBuffCountMoreThan" => buff::buff_type_pair_at_least, predicate(&[]);
    [536208] "TypeIdBuffCountLessThan" => buff::buff_type_at_most, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [536201] "TypeIdBuffCountLessThan" => buff::buff_type_at_most, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [536210] "TypeIdBuffCountLessThan" => buff::buff_type_at_most, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [512032, 51004] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, predicate(&[EventKind::BuffChanged]);
    [51002] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, setup_route(SetupStage::EnterFight, 0, &[]);
    [51102] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, setup_route(SetupStage::RoundStartCondition, 102, &[]);
    [51103] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, setup_route(SetupStage::RoundStart, 1, &[]);
    [511201] "HasTypeBuffIdsMoreThan" => buff::buff_status_at_least, predicate(&[EventKind::BuffChanged]);
    [42208] "HasTypeBuffMoreThan" => buff::buff_status_at_least, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [512210] "HasTypeBuffIdsLessThan" => buff::buff_status_at_most, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [51213999] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, predicate(&[EventKind::BuffAdded, EventKind::BuffChanged]);
    [51104] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, setup_in_side_frame(setup_route(SetupStage::RoundStart, 4, &[EventKind::BuffChanged]));
    [51106] "HasTypeIdBuffMoreThan" => buff::buff_type_at_least, setup_route(SetupStage::CardSetup, 0, &[EventKind::BuffChanged]);
    [61201] "PerBuffIdCount" => buff::per_buff_id_count, uses_active_skill_targets(predicate(&[EventKind::BuffChanged]));
    [61102] "PerBuffIdCount" => buff::per_buff_id_count, setup_route(SetupStage::RoundStartCondition, 102, &[]);
    [61203] "PerBuffIdCount" => buff::per_buff_id_count, uses_active_skill_targets(predicate(&[EventKind::BuffChanged]));
    [59203] "PerBuffId" => buff::per_buff_id, predicate(&[EventKind::BuffChanged]);
    [59302] "PerBuffId" => buff::per_buff_id, event_trigger(EventKind::RoundEnd, None);
    [61208] "PerBuffIdCount" => buff::per_buff_id_count, uses_active_skill_targets(matching_buff_act_owns_behavior(event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage))));
    [61210] "PerBuffIdCount" => buff::per_buff_id_count, uses_active_skill_targets(event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit)));
    [85203] "PerBuffTypeCountGroupByTypeId" => buff::per_distinct_status_type_count, predicate(&[EventKind::BuffChanged]);
    [518203] "PerHasBuffTypeLayer" => buff::per_type_layer, predicate(&[EventKind::BuffChanged]);
    [518210] "PerHasBuffTypeLayer" => buff::per_type_layer, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [77203] "HasBuffGroup" => buff::buff_group, filters_behavior_targets(predicate(&[EventKind::BuffChanged]));
    [77208] "HasBuffGroup" => buff::buff_group, filters_behavior_targets(event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage)));
    [78208] "NoBuffGroup" => buff::no_buff_group, filters_behavior_targets(event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage)));
    [1007204] "FromBuffAndToBuff" => buff::from_and_to_buff, predicate(&[EventKind::BuffChanged]);
    [701201] "HasMasterHalo" => buff::master_halo, predicate(&[EventKind::BuffChanged]);
    [701203] "HasMasterHalo" => buff::master_halo, predicate(&[EventKind::BuffChanged]);
    [701210] "HasMasterHalo" => buff::master_halo, predicate(&[EventKind::BuffChanged]);
    [501203] "UseHurtSkill" => active_skill::hurt_skill, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [501201] "UseHurtSkill" => active_skill::hurt_skill, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [501208] "UseHurtSkill" => active_skill::hurt_skill, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [501210] "UseHurtSkill" => active_skill::hurt_skill, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [501212] "UseHurtSkill" => active_skill::hurt_skill, predicate(&[]);
    [507201] "UseSkillId" => active_skill::skill_id, event_trigger(EventKind::SkillAction, None);
    [615201] "CanUseSkill" => active_skill::can_use_skill, predicate(&[]);
    [578] "TeamInjuryCountRound" => injury::round_count, predicate(&[EventKind::SkillAction]);
    [629210] "TeammateInjuryCount" => injury::teammate_count, event_trigger(EventKind::SkillAction, None);
    [630212] "TeammateInjuryCountNotReset" => injury::persistent_teammate_count, event_trigger(EventKind::AllyAction, None);
    [618012] "TeammateAliveOrDyingNumNoSp" => entity_count::teammates_without_special, predicate(&[EventKind::EntityDied]);
    [616012] "TeammateAliveNumNoSp" => entity_count::teammates_without_special, predicate(&[EventKind::EntityDied]);
    [73301] "TeammateAliveNum" => entity_count::teammates_equal, event_trigger(EventKind::RoundEnd, None);
    [583004] "AccTeamAddBuffCountByBuffId" => buff::team_added_count, reaction_targets_owner(predicate(&[EventKind::BuffAdded, EventKind::BuffChanged]));
    [581] "AccAddBuffCountByBuffId" => buff::owner_added_count, reaction_targets_owner(predicate(&[EventKind::BuffAdded, EventKind::BuffChanged]));
    [581307] "AccAddBuffCountByBuffId" => buff::buff_id_at_least, event_trigger(EventKind::RoundEndFinalSettlement, None);
    [579018] "ExPointIncrChange" => resource::self_ex_point_increase, reaction_targets_owner(before_publish(event_trigger(EventKind::ExPointChanged, None)));
    [579023] "ExPointIncrChange" => resource::other_ally_ex_point_increase, reaction_targets_owner(before_publish(event_trigger(EventKind::ExPointChanged, None)));
    [40] "LostExPoint" => resource::ex_point_lost, event_trigger(EventKind::ExPointChanged, None);
    [566] "PowerUseAddBuff" => resource::power_use_add_buff, after_skill(reaction_targets_owner(event_trigger(EventKind::EurekaChanged, None)));
    [660008] "PerDecrExPoint" => resource::ex_point_decrease, before_publish(event_trigger(EventKind::ExPointChanged, None));
    [721017] "CurEntityPowerDel" => resource::current_entity_power_decrease, reaction_targets_owner(event_trigger(EventKind::EurekaChanged, None));
    [724304] "OverFlowPower" => resource::power_overflow, event_trigger(EventKind::RoundEndAfterSettlement, None);
    [725304] "ComsumePower" => resource::power_consumed, event_trigger(EventKind::RoundEndAfterSettlement, None);
    [726203] "BloodPoolValue" => resource::blood_pool_value, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [726103] "BloodPoolValue" => resource::blood_pool_value, event_trigger(EventKind::RoundEnd, None);
    [726210] "BloodPoolValue" => resource::blood_pool_value, predicate(&[]);
    [589] "PowerIncrChange" => resource::power_increase, reaction_targets_owner(event_trigger(EventKind::EurekaChanged, None));
    [749301] "PowerRatio" => resource::power_ratio, event_trigger(EventKind::SmallRoundEnd, None);
    [710301] "PerHandCardHasSkillId" => card::hand_skill_presence, event_trigger(EventKind::SmallRoundEnd, None);
    [571017] "LostPower" => resource::lost_power, reaction_targets_owner(event_trigger(EventKind::EurekaChanged, None));
    [788210] "PerDeviceCurrCost" => resource::per_conduit_current_cost, in_causing_frame(reaction_targets_owner(event_trigger(EventKind::ConduitActivated, None)));
    [787103] "DeviceExPoint" => conduit::ex_point, setup_route(SetupStage::RoundStart, 1, &[]);
    [787105] "DeviceExPoint" => conduit::ex_point, setup_route(SetupStage::AfterRoundStart, 0, &[]);
    [794103] "DeviceSkillIndex" => conduit::selected_group, setup_route(SetupStage::RoundStart, 1, &[]);
    [591, 592, 593] "None" => none::healed, event_trigger(EventKind::HpHealed, None);
    [613026, 613403] "PerTeamEntityExitCount" => lifecycle::team_entity_exited, event_trigger(EventKind::EntityDied, None);
    [520203, 520210] "SummonedNumMoreThan" => entity_count::summoned_at_least, predicate(&[EventKind::SummonChanged]);
    [525104] "SummonedNumEqual" => entity_count::summoned_equal, setup_route(SetupStage::RoundStart, 1, &[EventKind::SummonChanged]);
    [525203, 525210] "SummonedNumEqual" => entity_count::summoned_equal, predicate(&[EventKind::SummonChanged]);
    [525212] "SummonedNumEqual" => entity_count::summoned_equal, event_trigger(EventKind::AllyAction, None);
    [697101] "TeamLostHpPercent" => hp::team_lost_hp, setup_route(SetupStage::RoundStartCondition, 101, &[]);
    [522203, 522210] "GroupSummonedNumMoreThan" => entity_count::group_summoned_at_least, predicate(&[EventKind::SummonChanged]);
    [5462032] "EnemyNumIncludeSpMoreThan" => entity_count::enemies_with_special_at_least, predicate(&[]);
    [546208] "EnemyNumIncludeSpMoreThan" => entity_count::enemies_with_special_at_least, predicate(&[]);
    [548201] "EnemyNumIncludeSpEqual" => entity_count::enemies_with_special_equal, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [717210] "TargetCount" => entity_count::target_count, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [5472032] "EnemyNumIncludeSpLessThan" => entity_count::enemies_with_special_at_most, predicate(&[]);
    [1011201] "EnemyAliveNum" => entity_count::enemy_alive, event_trigger(EventKind::SkillAction, None);
    [1011208] "EnemyAliveNum" => entity_count::enemy_alive, predicate(&[]);
    [595002] "TargetIncludeHero" => target_identity::target_model, setup_route(SetupStage::EnterFight, 0, &[]);
    [1000212] "TeamContainHero" => target_identity::team_contains_model, predicate(&[]);
    [643004] "HasConditionTarget" => target_identity::team_model_presence, predicate(&[]);
    [585208] "TargetIsSelf" => target_identity::target_is_self, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [586208] "TargetIsTeamNoMe" => target_identity::target_is_ally_not_self, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [16002] "TargetCareer" => career::target_career, filters_behavior_targets(setup_route(SetupStage::EnterFight, 0, &[]));
    [16021] "TargetCareer" => career::target_career, setup_route(SetupStage::BattleStart, 0, &[]);
    [16204] "TargetCareer" => career::target_career, predicate(&[]);
    [762021] "BattleTagNum" => battle_tag::parse, setup_route(SetupStage::BattleStart, 0, &[]);
    [762103] "BattleTagNum" => battle_tag::parse, setup_route(SetupStage::RoundStart, 1, &[]);
    [760212] "CurUseCardEnchant" => card::current_enchant, event_trigger(EventKind::AllyAction, None);
    [760402] "CurUseCardEnchant" => card::current_enchant, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [16010, 16203] "TargetCareer" => career::target_career, predicate(&[]);
    [16210] "TargetCareer" => career::target_career, filters_behavior_targets(predicate(&[]));
    [36021] "HeroReal" => parse::hero_reality, setup_route(SetupStage::BattleStart, 0, &[]);
    [37021] "HeroMagic" => parse::hero_mental, setup_route(SetupStage::BattleStart, 0, &[]);
    [508104] "CareerCheck" => career::parse_career_check, setup_route(SetupStage::RoundStart, 1, &[]);
    [565104] "EnemyHighestTypeIdBuffCountMoreThan" => buff::enemy_highest_buff_type_at_least, predicate(&[]);
    [508208] "CareerCheck" => career::parse_career_check, predicate(&[]);
    [508212] "CareerCheck" => career::parse_career_check, event_trigger(EventKind::AllyAction, None);
    [650002, 650102] "PerHasTargetCareerList" => career::parse_per_target_career_count, setup_route(SetupStage::EnterFight, 0, &[]);
    [621002] "CareerNatureHeroNum" => career::natural_ally_count, setup_route(SetupStage::EnterFight, 0, &[]);
    [562002] "CareerGroupHeroCountGE" => career::team_career_count_at_least, setup_route(SetupStage::EnterFight, 0, &[]);
    [562101] "CareerGroupHeroCountGE" => career::team_career_count_at_least, setup_route(SetupStage::RoundStartCondition, 101, &[]);
    [560100] "CareerGroupHeroCountLE" => career::team_career_count_at_most, setup_route(SetupStage::RoundStartCondition, 100, &[]);
    [573002] "PerTeamOtherEntityDmgType" => entity_count::other_ally_damage_type, setup_route(SetupStage::EnterFight, 0, &[]);
    [17] "TeammateDead" => entity_count::teammate_dead, event_trigger(EventKind::EntityDied, None);
    [17012] "TeammateDead" => entity_count::teammate_dead, event_trigger(EventKind::EntityDied, None);
    [86] "EnemyDead" => entity_count::enemy_dead, event_trigger(EventKind::EntityDied, None);
    [11210] "SingleKillNum" => entity_count::single_kill, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [99210] "PerKillNum" => entity_count::per_kill, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [992101] "PerKillNum" => entity_count::per_kill, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [8] "Dead" => lifecycle::entity_dead, event_trigger(EventKind::EntityDied, None);
    [812] "Dead" => lifecycle::entity_dead, reaction_targets_owner(event_trigger(EventKind::EntityDied, None));
    [24102] "TeammateAlive" => entity_count::teammate_alive, event_trigger(EventKind::RoundStart, None);
    [524302] "GroupSummonedNumEqual" => entity_count::group_summoned_equal, event_trigger(EventKind::RoundEnd, None);
    [726304] "BloodPoolValue" => resource::blood_pool_value, event_trigger(EventKind::RoundEndAfterSettlement, None);
    [649, 649203, 649210] "TriggerBullet" => trigger::parse_buff_feature, event_trigger(EventKind::BuffFeatureTriggered, None);
    [
        741000, 741002, 741100, 741101, 741102, 741103, 741104, 741105, 741106,
        741201, 741202, 741203, 741204, 741205, 741206, 741207, 741208, 741209,
        741210, 741211, 741212, 741213, 741301, 741302, 741303, 741304, 741305,
        741306, 741307, 741401, 741402,
    ] "TriggerTypeBullet" => trigger::parse_buff_feature, event_trigger(EventKind::BuffFeatureTriggered, None);
    [46301, 46303, 46304, 46307] "NoActRound" => trigger::parse_no_action_round, event_trigger(EventKind::NoActionRound, None);
    [22202, 22204, 22209, 22211] "BeAttacked" => trigger::parse_target_attacked, event_trigger(EventKind::TargetAttacked, None);
    [22213] "BeAttacked" => trigger::parse_ally_attacked, ally_of_attacked_target_observes(event_trigger(EventKind::SkillAction, Some(SkillPhase::HitPassives)));
    [695213] "ShareDamage" => trigger::parse_share_damage, event_trigger(EventKind::TargetAttacked, None);
    [1001203] "Assassinate" => trigger::parse_assassinate, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [1001204] "Assassinate" => trigger::parse_assassinate, event_trigger(EventKind::SkillAction, None);
    [1001208] "Assassinate" => trigger::parse_assassinate, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [1001210] "Assassinate" => trigger::parse_assassinate, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [1001212] "Assassinate" => trigger::parse_assassinate, team_observes(event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit)));
    [791210] "ToBrokenEnemy" => trigger::parse_target_guard_broken, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [25203] "UseExSkill" => trigger::parse_use_ex_skill, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [25204] "UseExSkill" => trigger::parse_use_ex_skill, incoming_attack_modifier(attack_target_observes(event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate))));
    [25208] "UseExSkill" => trigger::parse_use_ex_skill, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [25210] "UseExSkill" => trigger::parse_use_ex_skill, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [564210] "BurnOverflow" => buff::burn_overflow, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [564203] "BurnOverflow" => buff::burn_overflow, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [25212] "UseExSkill" => trigger::parse_target_use_ex_skill, event_trigger(EventKind::AllyAction, None);
    [720212] "TeammateUseExSkill" => trigger::parse_teammate_use_ex_skill, event_trigger(EventKind::AllyAction, None);
    [502212] "ActiveUseSkill" => active_skill::active_use, normal_buff_grant(event_trigger(EventKind::AllyAction, None));
    [620212] "CurrSkillLevel" => active_skill::rank, event_trigger(EventKind::AllyAction, None);
    [502203] "ActiveUseSkill" => active_skill::active_use, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [502208] "ActiveUseSkill" => active_skill::active_use, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [502210] "ActiveUseSkill" => active_skill::active_use, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [659212] "UseSkill" => active_skill::use_skill, event_trigger(EventKind::AllyAction, None);
    [66203] "UseSpecificSkill" => active_skill::specific_skill, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [66208] "UseSpecificSkill" => active_skill::specific_skill, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [66209] "UseSpecificSkill" => active_skill::received_specific_skill, predicate(&[EventKind::TargetAttacked]);
    [66210] "UseSpecificSkill" => active_skill::specific_skill, predicate(&[EventKind::SkillAction]);
    [662201] "ActiveUseSkillId" => active_skill::skill_id, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [662203] "ActiveUseSkillId" => active_skill::skill_id, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [6622032] "ActiveUseSkillId" => active_skill::skill_id, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [662208] "ActiveUseSkillId" => active_skill::skill_id, uses_active_skill_targets(child_buff_grant(event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage))));
    [403201, 403208] "SkillExtraType" => extra::active_action, predicate(&[]);
    [403203] "SkillExtraType" => extra::active_action, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [403210] "SkillExtraType" => extra::active_action, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [403212] "SkillExtraType" => extra::other_ally_action, event_trigger(EventKind::AllyAction, None);
    [626212] "ActionSkillExtraType" => extra::other_ally_action, event_trigger(EventKind::AllyAction, None);
    [656212] "SelfBuffTypeTargetBuffTypes" => buff::self_buff_type_target_buff_types, event_trigger(EventKind::AllyAction, None);
    [180203] "PowerCompare" => resource::power_compare, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [180208] "PowerCompare" => resource::power_compare, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [180210] "PowerCompare" => resource::power_compare, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [180212999] "PowerCompare" => resource::power_compare, team_observes(event_trigger(EventKind::AllyAction, None));
    [180213999] "PowerCompare" => resource::power_compare, opposing_team_observes(event_trigger(EventKind::AllyAction, None));
    [180100] "PowerCompare" => resource::power_compare, setup_route(SetupStage::RoundStartCondition, 100, &[]);
    [180102] "PowerCompare" => resource::power_compare, setup_route(SetupStage::RoundStartCondition, 102, &[]);
    [180104] "PowerCompare" => resource::power_compare, setup_route(SetupStage::RoundStart, 1, &[]);
    [180106] "PowerCompare" => resource::power_compare, setup_route(SetupStage::CardSetup, 0, &[]);
    [89210] "ExpointMoreThan" => resource::ex_point_at_least, predicate(&[EventKind::ExPointChanged]);
    [1008101] "Synchronization" => resource::synchronization, predicate(&[EventKind::ExPointChanged]);
    [526203, 526210] "ExpointLessThan" => resource::ex_point_at_most, predicate(&[EventKind::ExPointChanged]);
    [544100] "NotInMagicCircleId" => magic_circle::absent, setup_route(SetupStage::RoundStartCondition, 100, &[EventKind::FieldChanged]);
    [542103, 542104] "InMagicCircleId" => magic_circle::present, setup_route(SetupStage::RoundStart, 1, &[EventKind::FieldChanged]);
    [542203] "InMagicCircleId" => magic_circle::present, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [542210] "InMagicCircleId" => magic_circle::present, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [542004] "InMagicCircleId" => magic_circle::present, predicate(&[EventKind::FieldChanged]);
    [711039] "AddMagicCircle" => magic_circle::added, event_trigger(EventKind::FieldChanged, None);
    [712040] "RemoveMagicCircle" => magic_circle::removed, event_trigger(EventKind::FieldChanged, None);
    [10] "BuffIdAdd" => buff::buff_added, event_trigger(EventKind::BuffChanged, None);
    [49] "BuffIdDel" => buff::buff_removed, event_trigger(EventKind::BuffRemoved, None);
    [510] "MultiHpXIn" => parse::multi_hp_segment, predicate(&[]);
    [552017] "Random" => parse::random, predicate(&[]);
    [552203] "Random" => parse::random, predicate(&[]);
    [552210] "Random" => parse::random, predicate(&[]);
    [34210] "UseSkillEffectTag" => active_skill::effect_tag, before_publish(event_trigger(EventKind::SkillAction, Some(SkillPhase::HitPassives)));
    [500203] "SkillType" => active_skill::skill_type, predicate(&[EventKind::SkillAction]);
    [500210] "SkillType" => active_skill::skill_type, predicate(&[EventKind::SkillAction]);
    [34203] "UseSkillEffectTag" => active_skill::effect_tag, event_trigger(EventKind::SkillEffectStarted, Some(SkillPhase::Immediate));
    [34212] "UseSkillEffectTag" => active_skill::effect_tag, predicate(&[]);
    [33201] "HurtRestraint" => parse::hurt_restrained, predicate(&[]);
    [33204] "HurtRestraint" => parse::hurt_restrained, incoming_attack_modifier(attack_target_observes(event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate))));
    [33209] "HurtRestraint" => parse::hurt_restrained, predicate(&[EventKind::TargetAttacked]);
    [47204] "HurtNotRestraint" => parse::hurt_not_restrained, incoming_attack_modifier(predicate(&[]));
    [47209] "HurtNotRestraint" => parse::hurt_not_restrained, predicate(&[EventKind::TargetAttacked]);
    [53201] "HurtNumType" => parse::damage_target_count_kind, predicate(&[]);
    [53210] "HurtNumType" => parse::damage_target_count_kind, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
    [20202] "HurtReal" => parse::reality_damage, incoming_attack_modifier(predicate(&[]));
    [20204] "HurtReal" => parse::reality_damage, incoming_attack_modifier(predicate(&[]));
    [20209] "HurtReal" => parse::reality_damage, predicate(&[EventKind::TargetAttacked]);
    [21204] "HurtMagic" => parse::mental_damage, incoming_attack_modifier(predicate(&[]));
    [21209] "HurtMagic" => parse::mental_damage, predicate(&[EventKind::TargetAttacked]);
    [538203] "EntityHurtMagic" => parse::mental_damage, predicate(&[]);
    [540203] "EntityHurtReal" => parse::reality_damage, predicate(&[]);
    [58201] "PerExPoint" => resource::per_ex_point, predicate(&[]);
    [1103] "LifeLess" => parse::hp_less, setup_route(SetupStage::RoundStart, 1, &[]);
    [1104] "LifeLess" => parse::hp_less, setup_route(SetupStage::RoundStartLate, 0, &[]);
    [1105] "LifeLess" => parse::hp_less, setup_route(SetupStage::AfterRoundStart, 0, &[]);
    [1203] "LifeLess" => parse::hp_less, predicate(&[]);
    [1204] "LifeLess" => parse::hp_less, incoming_attack_modifier(predicate(&[]));
    [1304] "LifeLess" => parse::hp_less, predicate(&[EventKind::HpLost]);
    [1209] "LifeLess" => parse::hp_less, predicate(&[EventKind::HpLost]);
    [2104] "LifeMore" => parse::hp_more, setup_route(SetupStage::RoundStartLate, 0, &[]);
    [2203] "LifeMore" => parse::hp_more, predicate(&[]);
    [2301] "LifeMore" => parse::hp_more, event_trigger(EventKind::SmallRoundEnd, None);
    [2304] "LifeMore" => parse::hp_more, predicate(&[EventKind::HpLost]);
    [744203] "PerHp" => hp::per_hp, predicate(&[EventKind::HpLost]);
    [12203] "LostLifePer" => hp::per_lost_hp, predicate(&[EventKind::HpLost]);
    [30208] "AttackCrit" => parse::attack_crit, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [30402] "AttackCrit" => parse::attack_crit, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [30210] "AttackCrit" => parse::attack_crit, predicate(&[]);
    [7203] "BeforeCrit" => parse::before_crit, event_trigger(EventKind::SkillAction, Some(SkillPhase::Damage));
    [740203] "BloodPoolMax" => resource::blood_pool_max, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [718212] "ActOrderRange" => act_order::range, event_trigger(EventKind::AllyAction, None);
    [35201] "ActOrder" => act_order::order, event_trigger(EventKind::SkillAction, None);
    [35203] "ActOrder" => act_order::order, event_trigger(EventKind::SkillAction, Some(SkillPhase::Immediate));
    [35208] "ActOrder" => act_order::order, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterDamage));
    [35210] "ActOrder" => act_order::order, event_trigger(EventKind::SkillAction, Some(SkillPhase::AfterHit));
}

/// Parses conditions only after an exact `(opcode, type_name)` registry match.
/// Evaluation consumes this predicate and remains separate from state mutation.
pub fn parse(opcode: i32, type_name: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let definition = find_key(opcode, type_name)?;
    (definition.parse)(opcode, type_name, args)
}

pub fn find_key(opcode: i32, type_name: &str) -> Option<&'static ConditionDefinition> {
    definitions().find(|definition| definition.key.matches(opcode, type_name))
}

pub fn definitions() -> impl Iterator<Item = &'static ConditionDefinition> {
    DEFINITIONS.iter()
}

pub fn conditions_filter_behavior_targets(conditions: &[ParsedCondition]) -> bool {
    super::query::find(conditions, &|condition| {
        find_key(condition.opcode, &condition.type_name)
            .is_some_and(|definition| definition.filters_behavior_targets)
    })
    .is_some()
}

pub fn conditions_use_active_skill_targets(conditions: &[ParsedCondition]) -> bool {
    super::query::find(conditions, &|condition| {
        find_key(condition.opcode, &condition.type_name).is_some_and(|definition| {
            definition.behavior_target_source == BehaviorTargetSource::ActiveSkillTargets
        })
    })
    .is_some()
}

pub fn attack_modifier_side(conditions: &[ParsedCondition]) -> Option<AttackModifierSide> {
    conditions
        .iter()
        .find_map(|condition| match &condition.kind {
            ParsedConditionKind::Any(groups) => {
                groups.iter().find_map(|group| attack_modifier_side(group))
            }
            _ => find_key(condition.opcode, &condition.type_name)
                .and_then(|definition| definition.attack_modifier_side),
        })
}

#[cfg(test)]
mod test;
