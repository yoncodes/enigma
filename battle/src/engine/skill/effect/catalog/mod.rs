use std::collections::{HashMap, HashSet, VecDeque};

use config::GameDB;
use sonettobuf::Fight;

use crate::engine::{
    event::subscription::SubscriptionKey,
    skill::{
        behavior::classify::{BehaviorKind, BehaviorSpec},
        condition::{ParsedConditionKind, parse_conditions, query},
        effect::slot::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot},
        target::TargetRequest,
    },
};

#[derive(Debug, Clone, Default)]
pub struct SkillEffectCatalog {
    effects: HashMap<i32, ParsedSkillEffect>,
    aliases: HashMap<i32, i32>,
    effect_tags: HashMap<i32, i32>,
    logic_targets: HashMap<i32, i32>,
    damage_rates: HashMap<i32, i32>,
    skill_types: HashMap<i32, i32>,
    skill_effect_types: HashMap<i32, i32>,
    extra_kinds: HashMap<i32, i32>,
    big_skill_points: HashMap<i32, i32>,
    big_skills: HashMap<i32, bool>,
    target_limits: HashMap<i32, i32>,
    reinforced_skills: HashMap<i32, i32>,
    reachable_buffs: HashSet<i32>,
    issues: HashMap<i32, Vec<RuleIssue>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuleIssue {
    pub effect_id: i32,
    pub slot: u8,
    pub opcode: Option<i32>,
    pub type_name: Option<String>,
    pub raw: String,
    pub reason: RuleIssueReason,
}

#[repr(i32)]
pub(crate) enum SkillEffectTag {
    RealityDamage = 1,
    MentalDamage = 2,
    Debuff = 3,
    Buff = 4,
    CounterSpell = 5,
    Heal = 6,
    Device = 14,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleIssueReason {
    MalformedBehavior,
    MissingBehavior,
    UnsupportedBehavior,
}

impl SkillEffectCatalog {
    pub fn insert(&mut self, effect: ParsedSkillEffect) {
        self.effects.insert(effect.skill_id, effect);
    }

    #[cfg(test)]
    pub(crate) fn insert_issue(&mut self, skill_id: i32, issue: RuleIssue) {
        self.issues.entry(skill_id).or_default().push(issue);
    }

    pub fn insert_alias(&mut self, skill_id: i32, effect_id: i32) {
        self.aliases.insert(skill_id, effect_id);
    }

    pub fn reinforced_skill(&self, skill_id: i32) -> Option<i32> {
        self.reinforced_skills.get(&skill_id).copied()
    }

    pub fn insert_effect_tag(&mut self, effect_id: i32, effect_tag: i32) {
        self.effect_tags.insert(effect_id, effect_tag);
    }

    pub fn insert_logic_target(&mut self, effect_id: i32, logic_target: i32) {
        self.logic_targets.insert(effect_id, logic_target);
    }

    pub fn insert_damage_rate(&mut self, effect_id: i32, damage_rate: i32) {
        self.damage_rates.insert(effect_id, damage_rate);
    }

    pub fn insert_extra_kind(&mut self, effect_id: i32, extra_kind: i32) {
        self.extra_kinds.insert(effect_id, extra_kind);
    }

    pub fn get(&self, skill_id: i32) -> Option<&ParsedSkillEffect> {
        self.aliases
            .get(&skill_id)
            .and_then(|effect_id| self.effects.get(effect_id))
            .or_else(|| self.effects.get(&skill_id))
    }

    pub fn random_skill_references(&self, skill_id: i32) -> Vec<i32> {
        let Some(effect) = self.get(skill_id) else {
            return Vec::new();
        };
        effect
            .slots
            .iter()
            .filter_map(|slot| {
                let definition = crate::engine::skill::behavior::registry::find(&slot.behavior)?;
                matches!(
                    definition.kind,
                    BehaviorKind::RandomUseSkill
                        | BehaviorKind::CrystalReuse
                        | BehaviorKind::RemoveBuffUseSkill
                )
                .then(|| (definition.references)(&slot.behavior).skills)
            })
            .flatten()
            .collect()
    }

    pub fn random_condition_keys(&self) -> Vec<(i32, i32)> {
        fn append(
            conditions: &[crate::engine::skill::condition::ParsedCondition],
            skill_id: i32,
            keys: &mut Vec<(i32, i32)>,
        ) {
            for condition in conditions {
                match &condition.kind {
                    ParsedConditionKind::Random { .. } => {
                        let key = (skill_id, condition.opcode);
                        if !keys.contains(&key) {
                            keys.push(key);
                        }
                    }
                    ParsedConditionKind::Any(groups) => {
                        for group in groups {
                            append(group, skill_id, keys);
                        }
                    }
                    ParsedConditionKind::Not(inner) => {
                        let nested = crate::engine::skill::condition::ParsedCondition {
                            opcode: condition.opcode,
                            type_name: condition.type_name.clone(),
                            kind: *inner.clone(),
                            raw_args: condition.raw_args.clone(),
                        };
                        append(std::slice::from_ref(&nested), skill_id, keys);
                    }
                    _ => {}
                }
            }
        }

        let mut keys = Vec::new();
        for effect in self.effects.values() {
            for slot in &effect.slots {
                append(&slot.conditions, effect.skill_id, &mut keys);
            }
        }
        keys
    }

    pub fn effect_tag(&self, skill_id: i32) -> i32 {
        let effect_id = self.aliases.get(&skill_id).copied().unwrap_or(skill_id);
        self.effect_tags
            .get(&effect_id)
            .copied()
            .unwrap_or_default()
    }

    pub fn is_attack(&self, skill_id: i32) -> bool {
        self.damage_rate(skill_id) > 0
            || matches!(
                self.effect_tag(skill_id),
                tag if tag == SkillEffectTag::RealityDamage as i32
                    || tag == SkillEffectTag::MentalDamage as i32
            )
    }

    pub fn grants_resource_on_card_play(&self, skill_id: i32) -> bool {
        !self.get(skill_id).is_some_and(|effect| {
            effect.slots.iter().any(|slot| {
                crate::engine::skill::behavior::registry::find(&slot.behavior).is_some_and(
                    |definition| {
                        definition.card_play_role
                            == crate::engine::skill::behavior::registry::CardPlayRole::QueuePreparation
                    },
                )
            })
        })
    }

    pub fn is_assassinate(&self, skill_id: i32) -> bool {
        self.get(skill_id).is_some_and(|effect| {
            effect.slots.iter().any(|slot| {
                crate::engine::skill::behavior::registry::find(&slot.behavior)
                    .is_some_and(|definition| definition.kind == BehaviorKind::Assassinate)
            })
        })
    }

    pub fn logic_target(&self, skill_id: i32) -> i32 {
        let effect_id = self.aliases.get(&skill_id).copied().unwrap_or(skill_id);
        self.logic_targets
            .get(&effect_id)
            .copied()
            .unwrap_or_default()
    }

    pub fn damage_rate(&self, skill_id: i32) -> i32 {
        let effect_id = self.aliases.get(&skill_id).copied().unwrap_or(skill_id);
        self.damage_rates
            .get(&effect_id)
            .copied()
            .unwrap_or_default()
    }

    pub fn is_passive(&self, skill_id: i32) -> bool {
        let effect_id = self.aliases.get(&skill_id).copied().unwrap_or(skill_id);
        self.skill_types
            .get(&effect_id)
            .is_some_and(|skill_type| *skill_type != 0)
    }

    pub fn is_active(&self, skill_id: i32) -> bool {
        let effect_id = self.aliases.get(&skill_id).copied().unwrap_or(skill_id);
        self.skill_types.get(&effect_id) == Some(&0)
    }

    pub fn skill_type(&self, skill_id: i32) -> i32 {
        let effect_id = self.aliases.get(&skill_id).copied().unwrap_or(skill_id);
        self.skill_effect_types
            .get(&effect_id)
            .copied()
            .unwrap_or_default()
    }

    pub fn extra_kind(&self, skill_id: i32) -> i32 {
        let effect_id = self.aliases.get(&skill_id).copied().unwrap_or(skill_id);
        self.extra_kinds
            .get(&effect_id)
            .copied()
            .unwrap_or_default()
    }

    pub fn big_skill_point(&self, skill_id: i32) -> i32 {
        let effect_id = self.aliases.get(&skill_id).copied().unwrap_or(skill_id);
        self.big_skill_points
            .get(&effect_id)
            .copied()
            .unwrap_or_default()
    }

    pub fn is_big_skill(&self, skill_id: i32) -> bool {
        let effect_id = self.aliases.get(&skill_id).copied().unwrap_or(skill_id);
        self.big_skills.get(&effect_id).copied().unwrap_or_default()
    }

    pub fn target_limit(&self, skill_id: i32) -> usize {
        let effect_id = self.aliases.get(&skill_id).copied().unwrap_or(skill_id);
        self.target_limits
            .get(&effect_id)
            .copied()
            .unwrap_or_default()
            .max(0) as usize
    }

    pub fn condition_kind(
        &self,
        skill_id: i32,
        key: crate::engine::skill::rule::DefinitionKey,
    ) -> Option<&ParsedConditionKind> {
        self.get(skill_id)?
            .slots
            .iter()
            .find_map(|slot| {
                query::find(&slot.conditions, &|condition| {
                    key.matches(condition.opcode, &condition.type_name)
                })
            })
            .map(|condition| &condition.kind)
    }

    pub fn round_limit_for_condition(
        &self,
        skill_id: i32,
        key: crate::engine::skill::rule::DefinitionKey,
    ) -> i32 {
        self.get(skill_id)
            .and_then(|effect| {
                effect
                    .slots
                    .iter()
                    .find(|slot| {
                        query::find(&slot.conditions, &|condition| {
                            key.matches(condition.opcode, &condition.type_name)
                        })
                        .is_some()
                    })
                    .map(|slot| slot.round_limit)
            })
            .unwrap_or_default()
    }

    pub fn limit_for_condition(
        &self,
        skill_id: i32,
        key: crate::engine::skill::rule::DefinitionKey,
    ) -> i32 {
        self.get(skill_id)
            .and_then(|effect| {
                effect
                    .slots
                    .iter()
                    .find(|slot| {
                        query::find(&slot.conditions, &|condition| {
                            key.matches(condition.opcode, &condition.type_name)
                        })
                        .is_some()
                    })
                    .map(|slot| slot.limit)
            })
            .unwrap_or_default()
    }

    pub fn subscriptions(&self, skill_id: i32) -> Vec<SubscriptionKey> {
        let Some(effect) = self.get(skill_id) else {
            return Vec::new();
        };
        let mut subscriptions = Vec::new();
        for slot in &effect.slots {
            for subscription in slot.subscriptions() {
                if !subscriptions.contains(&subscription) {
                    subscriptions.push(subscription);
                }
            }
        }
        subscriptions
    }

    pub fn compiled_subscriptions(
        &self,
        skill_id: i32,
    ) -> Result<Vec<SubscriptionKey>, crate::engine::skill::rule::route::RouteError> {
        let Some(effect) = self.get(skill_id) else {
            return Ok(Vec::new());
        };
        let mut subscriptions = Vec::new();
        for slot in &effect.slots {
            for subscription in slot.compiled_subscriptions()? {
                if !subscriptions.contains(&subscription) {
                    subscriptions.push(subscription);
                }
            }
        }
        Ok(subscriptions)
    }

    pub fn compiled_subscription_lanes(
        &self,
        skill_id: i32,
    ) -> Result<Vec<(usize, SubscriptionKey)>, crate::engine::skill::rule::route::RouteError> {
        let Some(effect) = self.get(skill_id) else {
            return Ok(Vec::new());
        };
        let mut lanes = Vec::new();
        for (slot_index, slot) in effect.slots.iter().enumerate() {
            for subscription in slot.compiled_subscriptions()? {
                if !lanes
                    .iter()
                    .any(|(known_slot, known): &(usize, SubscriptionKey)| {
                        *known_slot == slot_index
                            && known.event == subscription.event
                            && known.phase == subscription.phase
                    })
                {
                    lanes.push((slot_index, subscription));
                }
            }
        }
        Ok(lanes)
    }

    pub fn issues(&self, skill_id: i32) -> &[RuleIssue] {
        let effect_id = self.aliases.get(&skill_id).copied().unwrap_or(skill_id);
        self.issues
            .get(&effect_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

mod build;
mod parse;

pub use parse::{
    configured_big_skill_point, configured_effect_id, configured_effect_tag, configured_extra_kind,
    configured_is_attack, configured_is_big_skill, configured_skill_type, global,
};
#[cfg(test)]
use parse::{parse_i32_list, parse_target};

#[cfg(test)]
mod test;
