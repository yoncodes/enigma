use std::collections::{HashMap, HashSet, VecDeque};

use config::GameDB;
use sonettobuf::Fight;

use crate::engine::{
    event::subscription::SubscriptionKey,
    skill::{
        behavior::classify::{BehaviorKind, BehaviorSpec},
        condition::{ParsedConditionKind, parse_conditions, query},
        effect::slot::{ParsedBehavior, ParsedSkillEffect, RuleLimitOwner, SkillEffectSlot},
        rule::DefinitionKey,
        target::TargetRequest,
    },
};

#[derive(Debug, Clone, Default)]
pub struct SkillEffectCatalog {
    effects: HashMap<i32, ParsedSkillEffect>,
    subscription_lanes: HashMap<i32, Result<Vec<(usize, SubscriptionKey)>, crate::engine::skill::rule::route::RouteError>>,
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
    assassination_stack_grants: std::collections::BTreeMap<(i32, i32), Vec<(i32, DefinitionKey)>>,
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
    MalformedTarget,
    MissingBehavior,
    MissingSummoned,
    UnsupportedBehavior,
}

impl RuleIssueReason {
    pub const fn is_blocking(self) -> bool {
        !matches!(self, Self::UnsupportedBehavior)
    }
}

impl SkillEffectCatalog {
    pub fn insert(&mut self, effect: ParsedSkillEffect) {
        self.subscription_lanes.insert(effect.skill_id, Self::compile_subscription_lanes(&effect));
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

    pub(crate) fn rule_limit_owner(&self, skill_id: i32, slot_index: usize) -> RuleLimitOwner {
        let Some(effect) = self.get(skill_id) else {
            return RuleLimitOwner::from_slot(slot_index);
        };
        let Some(slot) = effect.slots.get(slot_index) else {
            return RuleLimitOwner::from_slot(slot_index);
        };
        if slot.limit <= 0 && slot.round_limit <= 0 {
            return RuleLimitOwner::from_slot(slot_index);
        }
        let Some(threshold) = alternative_life_less_threshold(slot) else {
            return RuleLimitOwner::from_slot(slot_index);
        };
        let owner = effect.slots[..slot_index]
            .iter()
            .position(|candidate| {
                candidate.limit == slot.limit
                    && candidate.round_limit == slot.round_limit
                    && candidate.behavior.spec == slot.behavior.spec
                    && candidate.behavior.args == slot.behavior.args
                    && candidate.behavior.raw_args == slot.behavior.raw_args
                    && candidate.behavior.config_effect == slot.behavior.config_effect
                    && candidate.condition_target == slot.condition_target
                    && candidate.target == slot.target
                    && candidate.target_from_condition == slot.target_from_condition
                    && alternative_life_less_threshold(candidate) == Some(threshold)
            })
            .unwrap_or(slot_index);
        RuleLimitOwner::from_slot(owner)
    }

    pub fn random_skill_choice_references(&self, skill_id: i32) -> Vec<i32> {
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
                    BehaviorKind::RandomUseSkill | BehaviorKind::RemoveBuffUseSkill
                )
                .then(|| (definition.references)(&slot.behavior).skills)
            })
            .flatten()
            .collect()
    }

    pub fn crystal_reuse_references(&self, skill_id: i32) -> Vec<i32> {
        let Some(effect) = self.get(skill_id) else {
            return Vec::new();
        };
        effect
            .slots
            .iter()
            .filter_map(|slot| {
                let definition = crate::engine::skill::behavior::registry::find(&slot.behavior)?;
                (definition.kind == BehaviorKind::CrystalReuse
                    && definition
                        .supports
                        .is_none_or(|supports| supports(&slot.behavior)))
                .then(|| (definition.references)(&slot.behavior).skills)
            })
            .flatten()
            .collect()
    }

    pub fn direct_skill_target_code_for_child(
        &self,
        skill_id: i32,
        child_skill_id: i32,
    ) -> Option<i32> {
        let mut target_codes = self.get(skill_id)?.slots.iter().filter_map(|slot| {
            let definition = crate::engine::skill::behavior::registry::find(&slot.behavior)?;
            (definition.kind == BehaviorKind::DirectUseSkill
                && (definition.references)(&slot.behavior)
                    .skills
                    .contains(&child_skill_id))
            .then_some(slot.target.code)
        });
        let target_code = target_codes.next()?;
        target_codes
            .all(|candidate| candidate == target_code)
            .then_some(target_code)
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

    pub fn is_interval(&self, skill_id: i32) -> bool {
        self.get(skill_id).is_some_and(|effect| {
            effect
                .slots
                .iter()
                .any(|slot| slot.behavior.spec.kind == BehaviorKind::StopConduitSkill)
        })
    }

    pub(crate) fn assassination_stack_grants(
        &self,
        field_buff_id: i32,
    ) -> impl Iterator<Item = (i32, i32, DefinitionKey)> + '_ {
        self.assassination_stack_grants
            .iter()
            .filter(move |((field, _), _)| *field == field_buff_id)
            .flat_map(|((_, active), grants)| {
                grants
                    .iter()
                    .map(move |&(buff_id, key)| (*active, buff_id, key))
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
    ) -> Result<&[(usize, SubscriptionKey)], crate::engine::skill::rule::route::RouteError> {
        let Some(effect) = self.get(skill_id) else {
            return Ok(&[]);
        };
        self.subscription_lanes[&effect.skill_id].as_deref().map_err(Clone::clone)
    }

    fn compile_subscription_lanes(
        effect: &ParsedSkillEffect,
    ) -> Result<Vec<(usize, SubscriptionKey)>, crate::engine::skill::rule::route::RouteError> {
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

fn alternative_life_less_threshold(slot: &SkillEffectSlot) -> Option<i32> {
    let [condition] = slot.conditions.as_slice() else {
        return None;
    };
    if condition.type_name != "LifeLess" || !matches!(condition.opcode, 1104 | 1209 | 1304) {
        return None;
    }
    match condition.kind {
        ParsedConditionKind::HpPermille {
            compare: crate::engine::skill::condition::ConditionCompare::LessThan,
            threshold,
        } => Some(threshold),
        _ => None,
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
mod tests;
