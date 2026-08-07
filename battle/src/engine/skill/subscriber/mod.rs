use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    manager::BattleManagers,
    skill::{
        buff_act,
        effect::{SkillEffectCatalog, SkillEffectSlot},
        rule::{DefinitionKey, SetupStage, route::RouteError},
        target::{TargetEntity, TargetPool},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillSubscriber {
    pub owner_uid: i64,
    pub skill_id: i32,
    pub slot_index: Option<usize>,
    pub key: SubscriptionKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SetupSubscriber {
    pub owner_uid: i64,
    pub skill_id: i32,
    pub slot_index: usize,
    pub stage: SetupStage,
    pub priority: i32,
    pub key: DefinitionKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffActSubscriber {
    pub owner_uid: i64,
    pub source_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub team_type: i32,
    pub owner_alive: bool,
    pub amount: i32,
    pub key: SubscriptionKey,
    pub act_type: String,
    pub effect_time: i32,
    pub effect_condition: i32,
    pub args: Vec<i32>,
    pub raw: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffActSetupSubscriber {
    pub feature: crate::engine::manager::buff::ActiveBuffFeature,
    pub stage: SetupStage,
    pub priority: i32,
    pub key: DefinitionKey,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventSubscribers {
    pub skills: Vec<SkillSubscriber>,
    pub buff_acts: Vec<BuffActSubscriber>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriberError {
    MissingSkill { owner_uid: i64, skill_id: i32 },
    UncompiledRoute { skill_id: i32, route: RouteError },
}

pub fn for_entity(
    entity: &TargetEntity,
    catalog: &SkillEffectCatalog,
    event: EventKind,
) -> Vec<SkillSubscriber> {
    for_skills(entity.uid, &entity.passive_skills, catalog, event)
}

pub fn active_skills(pool: &TargetPool, managers: &BattleManagers) -> Vec<(i64, i32)> {
    let mut skills = Vec::new();
    for entity in pool.active_entities() {
        skills.extend(entity_skill_owners(entity, pool, managers));
    }
    skills.extend(additional_skill_owners(pool, managers));
    skills.sort_unstable();
    skills.dedup();
    skills
}

pub fn for_skills(
    owner_uid: i64,
    skill_ids: &[i32],
    catalog: &SkillEffectCatalog,
    event: EventKind,
) -> Vec<SkillSubscriber> {
    skill_ids
        .iter()
        .flat_map(|&skill_id| {
            catalog
                .subscriptions(skill_id)
                .into_iter()
                .filter(move |key| key.event == event)
                .map(move |key| SkillSubscriber {
                    owner_uid,
                    skill_id,
                    slot_index: None,
                    key,
                })
        })
        .collect()
}

pub fn for_active_buffs(managers: &BattleManagers, event: EventKind) -> Vec<BuffActSubscriber> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter_map(|feature| {
            let (&act_id, args) = feature.values.split_first()?;
            if !buff_act::registry::subscribes_to_event(
                act_id,
                &feature.act_type,
                feature.effect_time,
                event,
            ) {
                return None;
            }

            let phase = buff_act::registry::runtime_phase(
                act_id,
                &feature.act_type,
                feature.effect_time,
                event,
            );
            Some(BuffActSubscriber {
                owner_uid: feature.owner_uid,
                source_uid: feature.source_uid,
                buff_uid: feature.buff_uid,
                buff_id: feature.buff_id,
                team_type: feature.team_type,
                owner_alive: feature.owner_alive,
                amount: feature.amount,
                key: SubscriptionKey::at_phase_and_publication(
                    event,
                    buff_act::registry::find(act_id, &feature.act_type)?.key,
                    phase,
                    buff_act::registry::runtime_publication(act_id, &feature.act_type, event),
                ),
                act_type: feature.act_type,
                effect_time: feature.effect_time,
                effect_condition: feature.effect_condition,
                args: args.to_vec(),
                raw: feature.raw,
            })
        })
        .collect()
}

pub fn active_buffs_for_owners(
    managers: &BattleManagers,
    event: EventKind,
    owner_uids: &[i64],
) -> Vec<BuffActSubscriber> {
    let mut subscribers = for_active_buffs(managers, event);
    retain_in_owner_order(&mut subscribers, owner_uids, |subscriber| {
        subscriber.owner_uid
    });
    subscribers
}

pub fn buff_acts_for_setup_stage(
    managers: &BattleManagers,
    stage: SetupStage,
    priority: i32,
) -> Vec<BuffActSetupSubscriber> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter_map(|feature| {
            let (&act_id, _) = feature.values.split_first()?;
            let definition = buff_act::registry::find(act_id, &feature.act_type)?;
            definition
                .setup
                .routes
                .contains(&(stage, priority))
                .then_some(BuffActSetupSubscriber {
                    feature,
                    stage,
                    priority,
                    key: definition.key,
                })
        })
        .collect()
}

pub fn for_damage_calculation(managers: &BattleManagers) -> Vec<BuffActSubscriber> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            buff_act::effect_time::classify(feature.effect_time)
                == buff_act::effect_time::BuffActEvent::DamageCalculation
        })
        .filter_map(|feature| {
            let (&act_id, args) = feature.values.split_first()?;
            Some(BuffActSubscriber {
                owner_uid: feature.owner_uid,
                source_uid: feature.source_uid,
                buff_uid: feature.buff_uid,
                buff_id: feature.buff_id,
                team_type: feature.team_type,
                owner_alive: feature.owner_alive,
                amount: feature.amount,
                key: SubscriptionKey::new(
                    EventKind::DamageCalculation,
                    buff_act::registry::find(act_id, &feature.act_type)?.key,
                ),
                act_type: feature.act_type,
                effect_time: feature.effect_time,
                effect_condition: feature.effect_condition,
                args: args.to_vec(),
                raw: feature.raw,
            })
        })
        .collect()
}

#[cfg(test)]
pub fn for_event(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    event: EventKind,
) -> EventSubscribers {
    let skills = collect_event_skills(pool, managers, catalog, event, |skill_id| {
        Ok(catalog
            .subscriptions(skill_id)
            .into_iter()
            .map(|key| (None, key))
            .collect())
    })
    .expect("legacy subscription lookup is infallible");
    EventSubscribers {
        skills,
        buff_acts: for_active_buffs(managers, event),
    }
}

pub fn for_compiled_event(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    event: EventKind,
) -> Result<EventSubscribers, SubscriberError> {
    Ok(EventSubscribers {
        skills: collect_event_skills(pool, managers, catalog, event, |skill_id| {
            catalog.compiled_subscription_lanes(skill_id).map(|lanes| {
                lanes
                    .into_iter()
                    .map(|(slot, key)| (Some(slot), key))
                    .collect()
            })
        })?,
        buff_acts: for_active_buffs(managers, event),
    })
}

pub fn for_compiled_events(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    events: impl IntoIterator<Item = EventKind>,
) -> Result<EventSubscribers, SubscriberError> {
    let mut merged = EventSubscribers::default();
    for event in events {
        let subscribers = for_compiled_event(pool, managers, catalog, event)?;
        for skill in subscribers.skills {
            if !merged.skills.contains(&skill) {
                merged.skills.push(skill);
            }
        }
        for buff_act in subscribers.buff_acts {
            if !merged.buff_acts.contains(&buff_act) {
                merged.buff_acts.push(buff_act);
            }
        }
    }
    Ok(merged)
}

pub fn for_compiled_owner_events(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    events: impl IntoIterator<Item = EventKind>,
    owner_uids: &[i64],
) -> Result<EventSubscribers, SubscriberError> {
    let mut subscribers = for_compiled_events(pool, managers, catalog, events)?;
    retain_in_owner_order(&mut subscribers.skills, owner_uids, |subscriber| {
        subscriber.owner_uid
    });
    retain_in_owner_order(&mut subscribers.buff_acts, owner_uids, |subscriber| {
        subscriber.owner_uid
    });
    Ok(subscribers)
}

fn retain_in_owner_order<T>(items: &mut Vec<T>, owner_uids: &[i64], owner: impl Fn(&T) -> i64) {
    items.retain(|item| owner_uids.contains(&owner(item)));
    items.sort_by_key(|item| {
        owner_uids
            .iter()
            .position(|owner_uid| *owner_uid == owner(item))
            .expect("retained owner is present")
    });
}

#[cfg(test)]
mod owner_order_tests {
    use super::retain_in_owner_order;

    #[test]
    fn owner_scope_preserves_registration_order_within_each_owner() {
        let mut items = vec![(-1, 726), (-2, 726), (-1, 1048), (-3, 726)];

        retain_in_owner_order(&mut items, &[-1, -2], |(owner_uid, _)| *owner_uid);

        assert_eq!(items, vec![(-1, 726), (-1, 1048), (-2, 726)]);
    }
}

fn collect_event_skills(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    event: EventKind,
    subscriptions: impl Fn(i32) -> Result<Vec<(Option<usize>, SubscriptionKey)>, RouteError>,
) -> Result<Vec<SkillSubscriber>, SubscriberError> {
    let mut skills = Vec::new();
    for entity in pool.active_entities() {
        for (owner_uid, skill_id) in entity_skill_owners(entity, pool, managers) {
            push_skill_subscribers(
                &mut skills,
                owner_uid,
                &[skill_id],
                catalog,
                event,
                &subscriptions,
            )?;
        }
    }
    for (owner_uid, skill_id) in additional_skill_owners(pool, managers) {
        push_skill_subscribers(
            &mut skills,
            owner_uid,
            &[skill_id],
            catalog,
            event,
            &subscriptions,
        )?;
    }
    Ok(skills)
}

fn push_skill_subscribers(
    subscribers: &mut Vec<SkillSubscriber>,
    owner_uid: i64,
    skill_ids: &[i32],
    catalog: &SkillEffectCatalog,
    event: EventKind,
    subscriptions: &impl Fn(i32) -> Result<Vec<(Option<usize>, SubscriptionKey)>, RouteError>,
) -> Result<(), SubscriberError> {
    for &skill_id in skill_ids {
        if catalog.get(skill_id).is_none() {
            return Err(SubscriberError::MissingSkill {
                owner_uid,
                skill_id,
            });
        }
        let keys = subscriptions(skill_id)
            .map_err(|route| SubscriberError::UncompiledRoute { skill_id, route })?;
        for (slot_index, key) in keys.into_iter().filter(|(_, key)| key.event == event) {
            let subscriber = SkillSubscriber {
                owner_uid,
                skill_id,
                slot_index,
                key,
            };
            if !subscribers.contains(&subscriber) {
                subscribers.push(subscriber);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
pub fn for_round_start_priority(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    priority: i32,
) -> Vec<SkillSubscriber> {
    for_setup_stage(pool, managers, catalog, SetupStage::RoundStart, priority)
        .into_iter()
        .map(|subscriber| SkillSubscriber {
            owner_uid: subscriber.owner_uid,
            skill_id: subscriber.skill_id,
            slot_index: Some(subscriber.slot_index),
            key: SubscriptionKey::new(EventKind::RoundStart, subscriber.key),
        })
        .collect()
}

#[cfg(test)]
pub fn for_setup_stage(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    stage: SetupStage,
    priority: i32,
) -> Vec<SetupSubscriber> {
    collect_setup_stage(pool, managers, catalog, stage, priority, |slot| {
        Ok(slot.setup_keys(stage, priority))
    })
    .expect("legacy setup opcode lookup is infallible")
}

pub fn for_compiled_setup_stage(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    stage: SetupStage,
    priority: i32,
) -> Result<Vec<SetupSubscriber>, SubscriberError> {
    collect_setup_stage(pool, managers, catalog, stage, priority, |slot| {
        slot.compiled_setup_keys(stage, priority)
    })
}

fn collect_setup_stage(
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    stage: SetupStage,
    priority: i32,
    mut keys: impl FnMut(&SkillEffectSlot) -> Result<Vec<DefinitionKey>, RouteError>,
) -> Result<Vec<SetupSubscriber>, SubscriberError> {
    let mut skills = Vec::new();
    for entity in pool.active_entities() {
        skills.extend(entity_skill_owners(entity, pool, managers));
    }
    skills.extend(additional_skill_owners(pool, managers));

    let mut subscribers = Vec::new();
    for (owner_uid, skill_id) in skills {
        let effect = catalog.get(skill_id).ok_or(SubscriberError::MissingSkill {
            owner_uid,
            skill_id,
        })?;
        for (slot_index, slot) in effect.slots.iter().enumerate() {
            for key in
                keys(slot).map_err(|route| SubscriberError::UncompiledRoute { skill_id, route })?
            {
                let subscriber = SetupSubscriber {
                    owner_uid,
                    skill_id,
                    slot_index,
                    stage,
                    priority,
                    key,
                };
                if !subscribers.contains(&subscriber) {
                    subscribers.push(subscriber);
                }
            }
        }
    }
    subscribers.sort_by_key(|subscriber| !pool.source_is_attacker(subscriber.owner_uid));
    Ok(subscribers)
}

fn entity_skill_owners(
    entity: &TargetEntity,
    pool: &TargetPool,
    managers: &BattleManagers,
) -> Vec<(i64, i32)> {
    let mut skills = managers
        .entity
        .passive_skills(entity.uid)
        .into_iter()
        .flatten()
        .map(|&skill_id| (entity.uid, skill_id))
        .collect::<Vec<_>>();
    skills.extend(
        crate::engine::skill::behavior::magic_circle::active_self_skills(
            entity.uid, managers, pool,
        )
        .into_iter()
        .map(|skill_id| (entity.uid, skill_id)),
    );
    skills.extend(
        managers
            .buff
            .passive_skill_links_for(entity.uid)
            .into_iter()
            .map(|link| (link.owner_uid, link.skill_id)),
    );
    let mut seen = std::collections::HashSet::new();
    skills.retain(|skill| seen.insert(*skill));
    skills
}

fn additional_skill_owners(pool: &TargetPool, managers: &BattleManagers) -> Vec<(i64, i32)> {
    managers
        .entity
        .passive_overrides()
        .filter(|(owner_uid, _)| pool.entity(*owner_uid).is_none())
        .flat_map(|(owner_uid, passive_skills)| {
            passive_skills
                .iter()
                .map(move |&skill_id| (owner_uid, skill_id))
        })
        .chain(managers.battle_rule.owned_skills())
        .chain(managers.summon.active_unique_skills())
        .chain(pool.assist_boss_skill_owners())
        .collect()
}

#[cfg(test)]
mod test;
