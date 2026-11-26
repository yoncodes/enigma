use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    skill::condition::{
        ParsedCondition, ParsedConditionKind,
        registry::{self, ConditionRole},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConditionTiming {
    Static,
    Event(EventKind),
    Unknown,
}

impl ParsedCondition {
    pub fn timing(&self) -> ConditionTiming {
        registry::find_key(self.opcode, &self.type_name).map_or_else(
            || match self.kind {
                ParsedConditionKind::Unsupported(_) => ConditionTiming::Unknown,
                _ => ConditionTiming::Static,
            },
            |definition| match definition.role {
                ConditionRole::Trigger { event, .. } => ConditionTiming::Event(event),
                ConditionRole::Predicate | ConditionRole::Setup { .. } => ConditionTiming::Static,
            },
        )
    }

    pub fn subscriptions(&self) -> Vec<SubscriptionKey> {
        let mut subscriptions = Vec::new();
        self.collect_subscriptions(&mut subscriptions);
        subscriptions
    }

    fn collect_subscriptions(&self, subscriptions: &mut Vec<SubscriptionKey>) {
        if let Some(definition) = registry::find_key(self.opcode, &self.type_name) {
            match definition.role {
                ConditionRole::Trigger { event, phase } => push_unique(
                    subscriptions,
                    SubscriptionKey::at_phase(event, definition.key, phase)
                        .with_publication(definition.publication)
                        .with_timing(definition.reaction_timing.resolve(Default::default())),
                ),
                ConditionRole::Predicate => {
                    for event in definition.dependencies {
                        push_unique(
                            subscriptions,
                            SubscriptionKey::at_phase(
                                *event,
                                definition.key,
                                definition.dependency_phase,
                            )
                            .with_publication(definition.publication)
                            .with_timing(definition.reaction_timing.resolve(Default::default())),
                        );
                    }
                }
                ConditionRole::Setup { .. } => {}
            }
            return;
        }
        if let ParsedConditionKind::Any(groups) = &self.kind {
            for group in groups {
                if let Some(group_subscriptions) = group
                    .iter()
                    .map(ParsedCondition::subscriptions)
                    .find(|subscriptions| !subscriptions.is_empty())
                {
                    for subscription in group_subscriptions {
                        push_unique(subscriptions, subscription);
                    }
                }
            }
        }
    }
}

fn push_unique(subscriptions: &mut Vec<SubscriptionKey>, subscription: SubscriptionKey) {
    if !subscriptions.contains(&subscription) {
        subscriptions.push(subscription);
    }
}

#[cfg(test)]
mod tests;
