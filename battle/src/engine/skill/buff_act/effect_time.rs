use crate::engine::event::kind::EventKind;
use crate::engine::skill::action::SkillPhase;
use crate::engine::skill::rule::DefinitionKey;

pub const ROUND_END_ENTITY_SETTLEMENT: i32 = 303;
pub const ROUND_START_DURATION: i32 = 103;
pub const ROUND_START_CARD_STAGES: [i32; 2] = [105, 106];

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

effect_time_definitions! {
    0 => BuffActEvent::StaticRead,
    12 => BuffActEvent::Runtime(EventKind::EntityDied),
    101 => BuffActEvent::Runtime(EventKind::RoundStart),
    102 => BuffActEvent::Runtime(EventKind::RoundStart),
    103 => BuffActEvent::Runtime(EventKind::RoundStart),
    104 => BuffActEvent::Runtime(EventKind::RoundStart),
    105 => BuffActEvent::Runtime(EventKind::RoundStartCard),
    106 => BuffActEvent::Runtime(EventKind::RoundStartCard),
    201 => BuffActEvent::Runtime(EventKind::SkillAction); SkillPhase::Immediate,
    210 => BuffActEvent::Runtime(EventKind::SkillAction); SkillPhase::AfterHit,
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
    307 => BuffActEvent::Runtime(EventKind::RoundEndFinalSettlement),
    304 => BuffActEvent::Runtime(EventKind::RoundEndAfterSettlement),
    401 => BuffActEvent::Runtime(EventKind::Riposte),
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

pub fn duration_stage_for_skill_phase(phase: SkillPhase) -> Option<i32> {
    definitions()
        .find(|definition| definition.duration_phase == Some(phase))
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
    take_stage == ROUND_START_DURATION
        || take_stage == ROUND_END_ENTITY_SETTLEMENT
        || ROUND_START_CARD_STAGES.contains(&take_stage)
        || definition.duration_phase.is_some()
        || definition.event == BuffActEvent::Runtime(EventKind::SmallRoundEnd)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn keeps_round_end_channels_separate() {
        assert_eq!(classify(302), BuffActEvent::Runtime(EventKind::RoundEnd));
        assert_eq!(
            classify(303),
            BuffActEvent::Runtime(EventKind::RoundEndEntitySettlement)
        );
        assert_eq!(
            classify(304),
            BuffActEvent::Runtime(EventKind::RoundEndAfterSettlement)
        );
        assert_eq!(
            classify(305),
            BuffActEvent::Runtime(EventKind::ExPointOverflow)
        );
        assert_eq!(
            classify(307),
            BuffActEvent::Runtime(EventKind::RoundEndFinalSettlement)
        );
    }

    #[test]
    fn keeps_non_event_channels_out_of_runtime_events() {
        assert_eq!(classify(202), BuffActEvent::DamageCalculation);
        assert_eq!(classify(-1), BuffActEvent::CycleSubstitution);
        assert_eq!(classify(1051), BuffActEvent::CardChoice);
    }

    #[test]
    fn exact_effect_time_definitions_are_unique() {
        let definitions = definitions().collect::<Vec<_>>();
        let unique = definitions
            .iter()
            .map(|definition| definition.key.opcode)
            .collect::<HashSet<_>>();

        assert_eq!(definitions.len(), unique.len());
    }

    #[test]
    fn small_round_end_keeps_each_configured_duration_stage() {
        assert_eq!(
            duration_stages_for_event(EventKind::SmallRoundEnd).collect::<Vec<_>>(),
            vec![211, 301]
        );
    }

    #[test]
    fn duration_support_accepts_non_advancing_and_scheduled_policies() {
        assert!(supports_duration_policy(-1));
        assert!(supports_duration_policy(ROUND_START_DURATION));
        assert!(supports_duration_policy(210));
        assert!(supports_duration_policy(301));
        assert!(supports_duration_policy(ROUND_END_ENTITY_SETTLEMENT));
        assert!(!supports_duration_policy(209));
        assert!(!supports_duration_policy(205));
    }
}
