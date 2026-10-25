use crate::engine::{
    event::kind::EventKind,
    skill::{action::SkillPhase, rule::DefinitionKey},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PublicationPhase {
    BeforePublish,
    #[default]
    AfterPublish,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ReactionTiming {
    #[default]
    Immediate,
    AfterSettlement,
    AfterSkill,
    AfterHitSkills,
    AttackedSkills,
    HpThresholds,
    BeforeAfterHitObservers,
    AfterHitObservers,
    AfterHitShell,
    AfterSharedHit,
    AfterAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubscriptionKey {
    pub event: EventKind,
    pub definition: DefinitionKey,
    pub timing_policy: DefinitionKey,
    pub multiplicity_policy: DefinitionKey,
    pub phase: Option<SkillPhase>,
    pub publication: PublicationPhase,
    pub timing: ReactionTiming,
}

impl SubscriptionKey {
    pub const fn new(event: EventKind, definition: DefinitionKey) -> Self {
        Self {
            event,
            definition,
            timing_policy: definition,
            multiplicity_policy: definition,
            phase: None,
            publication: PublicationPhase::AfterPublish,
            timing: ReactionTiming::Immediate,
        }
    }

    pub const fn at_phase(
        event: EventKind,
        definition: DefinitionKey,
        phase: Option<SkillPhase>,
    ) -> Self {
        Self {
            event,
            definition,
            timing_policy: definition,
            multiplicity_policy: definition,
            phase,
            publication: PublicationPhase::AfterPublish,
            timing: ReactionTiming::Immediate,
        }
    }

    pub const fn with_publication(mut self, publication: PublicationPhase) -> Self {
        self.publication = publication;
        self
    }

    pub const fn with_timing(mut self, timing: ReactionTiming) -> Self {
        self.timing = timing;
        self
    }

    pub const fn with_timing_policy(mut self, timing_policy: DefinitionKey) -> Self {
        self.timing_policy = timing_policy;
        self
    }

    pub const fn with_multiplicity_policy(mut self, multiplicity_policy: DefinitionKey) -> Self {
        self.multiplicity_policy = multiplicity_policy;
        self
    }

    pub const fn at_phase_and_publication(
        event: EventKind,
        definition: DefinitionKey,
        phase: Option<SkillPhase>,
        publication: PublicationPhase,
    ) -> Self {
        Self {
            event,
            definition,
            timing_policy: definition,
            multiplicity_policy: definition,
            phase,
            publication,
            timing: ReactionTiming::Immediate,
        }
    }
}
