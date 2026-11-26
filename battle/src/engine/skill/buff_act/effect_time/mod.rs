use crate::engine::event::kind::EventKind;
use crate::engine::skill::action::SkillPhase;
use crate::engine::skill::rule::DefinitionKey;

pub const ROUND_END_ENTITY_SETTLEMENT: i32 = 303;
pub const ROUND_END_AFTER_SETTLEMENT: i32 = 304;
pub const ROUND_END_FINAL_SETTLEMENT: i32 = 307;
pub const ROUND_START_INITIAL_DURATION: i32 = 100;
pub const ROUND_START_ACTOR_DURATION: i32 = 101;
pub const ROUND_START_BEFORE_CONDITION_DURATION: i32 = 102;
pub const ROUND_START_CONDITION_DURATION_STAGES: [i32; 3] = [
    ROUND_START_INITIAL_DURATION,
    ROUND_START_ACTOR_DURATION,
    ROUND_START_BEFORE_CONDITION_DURATION,
];
pub const ROUND_START_DURATION: i32 = 103;
pub const ROUND_START_AFTER_REACTION_DURATION: i32 = 104;
pub const ROUND_START_DURATION_STAGES: [i32; 2] =
    [ROUND_START_DURATION, ROUND_START_AFTER_REACTION_DURATION];
pub const ROUND_START_CARD_REACTION_DURATION: i32 = 105;
pub const ROUND_START_CARD_SETTLED_DURATION: i32 = 106;
pub const ROUND_START_CARD_STAGES: [i32; 2] = [
    ROUND_START_CARD_REACTION_DURATION,
    ROUND_START_CARD_SETTLED_DURATION,
];
pub(crate) const AFTER_HIT_DURATION: i32 = 210;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuffActEvent {
    StaticRead,
    Runtime(EventKind),
    DamageCalculation,
    CycleSubstitution,
    ShellReaction,
    LayerThresholdSkill,
    ChannelResolution,
    CardChoice,
    CardRecord,
    CardCastChannel,
    Unknown(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectTimeDefinition {
    pub key: DefinitionKey,
    pub event: BuffActEvent,
    pub duration_phase: Option<SkillPhase>,
}

macro_rules! effect_time_definitions {
    ($($code:expr => $event:expr $(; $phase:expr)?),* $(,)?) => {
        pub const DEFINITIONS: &[EffectTimeDefinition] = &[
            $(EffectTimeDefinition {
                key: DefinitionKey::new($code, "EffectTime"),
                event: $event,
                duration_phase: effect_time_definitions!(@phase $($phase)?),
            }),*
        ];
    };
    (@phase) => { None };
    (@phase $phase:expr) => { Some($phase) };
}

pub(super) const RIPOSTE: i32 = 401;

effect_time_definitions! {
    0 => BuffActEvent::StaticRead,
    12 => BuffActEvent::Runtime(EventKind::EntityDied),
    ROUND_START_INITIAL_DURATION => BuffActEvent::Runtime(EventKind::RoundStart),
    ROUND_START_ACTOR_DURATION => BuffActEvent::Runtime(EventKind::RoundStart),
    102 => BuffActEvent::Runtime(EventKind::RoundStart),
    103 => BuffActEvent::Runtime(EventKind::RoundStart),
    104 => BuffActEvent::Runtime(EventKind::RoundStart),
    ROUND_START_CARD_REACTION_DURATION => BuffActEvent::Runtime(EventKind::RoundStartCard),
    ROUND_START_CARD_SETTLED_DURATION => BuffActEvent::Runtime(EventKind::RoundStartCard),
    107 => BuffActEvent::Runtime(EventKind::ActionQueueCommitted),
    201 => BuffActEvent::Runtime(EventKind::SkillAction); SkillPhase::Immediate,
    AFTER_HIT_DURATION => BuffActEvent::Runtime(EventKind::SkillAction); SkillPhase::AfterHit,
    208 => BuffActEvent::Runtime(EventKind::SkillCast),
    2081 => BuffActEvent::Runtime(EventKind::SkillCast),
    2101 => BuffActEvent::Runtime(EventKind::SkillCast),
    209 => BuffActEvent::Runtime(EventKind::BeAttacked),
    2091 => BuffActEvent::Runtime(EventKind::BeAttacked),
    2061 => BuffActEvent::Runtime(EventKind::BeAttacked),
    207 => BuffActEvent::Runtime(EventKind::BeAttackedDefense),
    211 => BuffActEvent::Runtime(EventKind::SmallRoundEnd),
    301 => BuffActEvent::Runtime(EventKind::SmallRoundEnd),
    212 => BuffActEvent::Runtime(EventKind::AllyAction),
    302 => BuffActEvent::Runtime(EventKind::RoundEnd),
    306 => BuffActEvent::Runtime(EventKind::RoundEnd),
    305 => BuffActEvent::Runtime(EventKind::ExPointOverflow),
    ROUND_END_ENTITY_SETTLEMENT => BuffActEvent::Runtime(EventKind::RoundEndEntitySettlement),
    ROUND_END_FINAL_SETTLEMENT => BuffActEvent::Runtime(EventKind::RoundEndFinalSettlement),
    ROUND_END_AFTER_SETTLEMENT => BuffActEvent::Runtime(EventKind::RoundEndAfterSettlement),
    RIPOSTE => BuffActEvent::Runtime(EventKind::Riposte),
    202 => BuffActEvent::DamageCalculation,
    203 => BuffActEvent::DamageCalculation,
    204 => BuffActEvent::DamageCalculation,
    900 => BuffActEvent::DamageCalculation,
    901 => BuffActEvent::DamageCalculation,
    903 => BuffActEvent::DamageCalculation,
    908 => BuffActEvent::DamageCalculation,
    -1 => BuffActEvent::CycleSubstitution,
    213 => BuffActEvent::ShellReaction,
    402 => BuffActEvent::LayerThresholdSkill,
    1041 => BuffActEvent::ChannelResolution,
    1051 => BuffActEvent::CardChoice,
    1061 => BuffActEvent::CardRecord,
    1062 => BuffActEvent::CardCastChannel,
}

pub fn definitions() -> impl Iterator<Item = &'static EffectTimeDefinition> {
    DEFINITIONS.iter()
}

pub fn find(code: i32) -> Option<&'static EffectTimeDefinition> {
    definitions().find(|definition| definition.key.opcode == code)
}

pub fn duration_stage_after_skill_phase(phase: SkillPhase) -> Option<i32> {
    let duration_phase = match phase {
        SkillPhase::HitPassives => SkillPhase::AfterHit,
        SkillPhase::AfterHit => return None,
        phase => phase,
    };
    definitions()
        .find(|definition| definition.duration_phase == Some(duration_phase))
        .map(|definition| definition.key.opcode)
}

pub fn classify(effect_time: i32) -> BuffActEvent {
    find(effect_time)
        .map(|definition| definition.event)
        .unwrap_or(BuffActEvent::Unknown(effect_time))
}

pub fn duration_stages_for_event(event: EventKind) -> impl Iterator<Item = i32> {
    definitions().filter_map(move |definition| {
        (definition.event == BuffActEvent::Runtime(event)).then_some(definition.key.opcode)
    })
}

pub fn supports_duration_policy(take_stage: i32) -> bool {
    if take_stage == -1 {
        return true;
    }
    let Some(definition) = find(take_stage) else {
        return false;
    };
    ROUND_START_CONDITION_DURATION_STAGES.contains(&take_stage)
        || ROUND_START_DURATION_STAGES.contains(&take_stage)
        || take_stage == ROUND_END_ENTITY_SETTLEMENT
        || take_stage == ROUND_END_AFTER_SETTLEMENT
        || take_stage == ROUND_END_FINAL_SETTLEMENT
        || ROUND_START_CARD_STAGES.contains(&take_stage)
        || definition.duration_phase.is_some()
        || definition.event == BuffActEvent::Runtime(EventKind::ActionQueueCommitted)
        || definition.event == BuffActEvent::Runtime(EventKind::AllyAction)
        || definition.event == BuffActEvent::Runtime(EventKind::SmallRoundEnd)
}

#[cfg(test)]
mod tests;
