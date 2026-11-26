use battle::tooling::{
    replay::{ConfiguredRuleKey, RandomBuffChoice},
    scan::{SkillEffectCatalog, behavior},
};
use sonettobuf::{
    ActEffect, FightRound, FightStep, effect_type_enum::EffectType, fight_step::ActType,
};

type KnownBuff = (i32, i64, u32);
type HeldBullets = HashMap<(i64, i64), KnownBuff>;

#[derive(Clone, Copy)]
struct Draw {
    id: i32,
    uid: i64,
    count: u32,
    expected: u32,
}

fn observed<'a>(
    effects: impl Iterator<Item = (&'a ActEffect, Option<KnownBuff>)>,
    source: i64,
    pool: &[(i32, u32)],
    count: usize,
) -> Option<Vec<i32>> {
    let mut draws = Vec::<Draw>::new();
    for (effect, before) in effects {
        let Some(kind) = effect
            .effect_type
            .and_then(|value| EffectType::try_from(value).ok())
        else {
            continue;
        };
        if effect.target_id != Some(source)
            || !matches!(kind, EffectType::Buffadd | EffectType::Buffupdate)
        {
            continue;
        }
        let Some(buff) = effect
            .buff
            .as_ref()
            .filter(|buff| buff.from_uid == Some(source))
        else {
            continue;
        };
        let Some((id, expected)) = pool
            .iter()
            .find(|(id, _)| buff.buff_id == Some(*id))
            .copied()
        else {
            continue;
        };
        let uid = buff.uid.filter(|uid| *uid != 0)?;
        let after = u32::try_from(buff.count?).ok()?;
        if kind == EffectType::Buffadd {
            if before.is_some()
                || after != 1
                || draws.iter().any(|draw| draw.id == id || draw.uid == uid)
            {
                return None;
            }
            draws.push(Draw {
                id,
                uid,
                count: after,
                expected,
            });
        } else if let Some(draw) = draws
            .iter_mut()
            .find(|draw| draw.id == id && draw.uid == uid)
        {
            if before != Some((id, source, draw.count))
                || Some(after) != draw.count.checked_add(1)
                || after > draw.expected
            {
                return None;
            }
            draw.count = after;
        } else {
            let (prior_id, applier, prior) = before?;
            if prior_id != id
                || applier != source
                || after != prior.checked_add(1)?
                || draws.iter().any(|draw| draw.id == id || draw.uid == uid)
            {
                return None;
            }
            draws.push(Draw {
                id,
                uid,
                count: after,
                expected: expected.checked_add(prior)?,
            });
        }
    }
    (draws.len() == count && draws.iter().all(|draw| draw.count == draw.expected))
        .then(|| draws.into_iter().map(|draw| draw.id).collect())
}

fn observe_buff(effect: &ActEffect, held: &mut HeldBullets) {
    let Some(kind) = effect
        .effect_type
        .and_then(|raw| EffectType::try_from(raw).ok())
    else {
        return;
    };
    if kind == EffectType::Changewave {
        held.clear();
    }
    let removal = matches!(kind, EffectType::Buffdel | EffectType::Buffdelnoeffect);
    if !removal && !matches!(kind, EffectType::Buffadd | EffectType::Buffupdate) {
        return;
    }
    let Some(target) = effect.target_id else {
        held.clear();
        return;
    };
    let Some((buff, uid)) = effect
        .buff
        .as_ref()
        .and_then(|buff| buff.uid.filter(|uid| *uid != 0).map(|uid| (buff, uid)))
    else {
        held.retain(|(owner, _), _| *owner != target);
        return;
    };
    held.remove(&(target, uid));
    if !removal
        && let Some(((id, source), count)) = buff.buff_id.zip(buff.from_uid).zip(
            buff.count
                .and_then(|count| u32::try_from(count).ok())
                .filter(|count| *count > 0),
        )
    {
        held.insert((target, uid), (id, source, count));
    }
}

pub(super) fn choices(catalog: &SkillEffectCatalog, round: &FightRound) -> Vec<RandomBuffChoice> {
    type Observations = HashMap<(ConfiguredRuleKey, i64), Option<RandomBuffChoice>>;
    fn visit(
        step: &FightStep,
        catalog: &SkillEffectCatalog,
        observations: &mut Observations,
        held: &mut HeldBullets,
    ) {
        if matches!(
            step.act_type.and_then(|kind| ActType::try_from(kind).ok()),
            Some(ActType::Changewave | ActType::Changehero)
        ) {
            held.clear();
        }
        let skill = step
            .act_id
            .and_then(|id| catalog.get(id))
            .zip(step.from_id.filter(|uid| *uid != 0))
            .filter(|_| step.act_type == Some(ActType::Skill as i32))
            .and_then(|(effect, source)| {
                let pools = effect
                    .slots
                    .iter()
                    .filter_map(|slot| {
                        behavior::random_arsenal_pool(&slot.behavior).map(|pool| (slot, pool))
                    })
                    .collect::<Vec<_>>();
                (!pools.is_empty()).then_some((effect, source, pools))
            });
        let mut chronology = Vec::new();
        for effect in &step.act_effect {
            if let Some(child) = effect.fight_step.as_ref() {
                visit(child, catalog, observations, held);
            }
            if skill.is_some() {
                let before = effect
                    .target_id
                    .zip(effect.buff.as_ref().and_then(|buff| buff.uid));
                chronology.push((effect, before.and_then(|key| held.get(&key).copied())));
            }
            observe_buff(effect, held);
        }
        if let Some((effect, source, pools)) = skill {
            for (slot, pool) in pools {
                let behavior = &slot.behavior;
                let Some(definition) = behavior::registry::find(behavior) else {
                    continue;
                };
                let rule =
                    ConfiguredRuleKey::new(behavior.effect_id, behavior.slot, definition.key);
                let count = usize::try_from(behavior.arg(1).unwrap_or_default())
                    .unwrap_or_default()
                    .min(pool.len());
                let choice = || {
                    let selected = observed(chronology.iter().copied(), source, &pool, count)?;
                    if effect
                        .slots
                        .iter()
                        .filter(|other| !std::ptr::eq(*other, slot))
                        .any(|other| {
                            let behavior = &other.behavior;
                            behavior::registry::find(behavior).is_none_or(|definition| {
                                (definition.references)(behavior)
                                    .buffs
                                    .iter()
                                    .any(|id| selected.contains(id))
                                    || behavior::random_buff_pool(behavior).is_some_and(|pool| {
                                        pool.iter().any(|id| selected.contains(id))
                                    })
                            })
                        })
                    {
                        return None;
                    }
                    Some(RandomBuffChoice {
                        rule,
                        source_uid: source,
                        target_uid: source,
                        buff_ids: selected,
                    })
                };
                observations
                    .entry((rule, source))
                    .and_modify(|existing| *existing = None)
                    .or_insert_with(choice);
            }
        }
    }

    let mut observations = Observations::new();
    let mut held = HeldBullets::new();
    for step in &round.fight_step {
        visit(step, catalog, &mut observations, &mut held);
    }
    observations.into_values().flatten().collect()
}

#[cfg(test)]
mod tests;
use std::collections::HashMap;
