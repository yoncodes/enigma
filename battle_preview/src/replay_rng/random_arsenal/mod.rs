use battle::tooling::{
    replay::{ConfiguredRuleKey, RandomBuffChoice},
    scan::{SkillEffectCatalog, behavior},
};
use sonettobuf::{FightRound, FightStep, effect_type_enum::EffectType, fight_step::ActType};

#[derive(Clone, Copy)]
struct Draw {
    id: i32,
    uid: i64,
    count: u32,
    expected: u32,
}

fn observed(step: &FightStep, source: i64, pool: &[(i32, u32)], count: usize) -> Option<Vec<i32>> {
    let mut draws = Vec::<Draw>::new();
    for effect in &step.act_effect {
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
            if after != 1 || draws.iter().any(|draw| draw.id == id || draw.uid == uid) {
                return None;
            }
            draws.push(Draw {
                id,
                uid,
                count: after,
                expected,
            });
        } else {
            let draw = draws
                .iter_mut()
                .find(|draw| draw.id == id && draw.uid == uid)?;
            if after != draw.count + 1 || after > draw.expected {
                return None;
            }
            draw.count = after;
        }
    }
    (draws.len() == count && draws.iter().all(|draw| draw.count == draw.expected))
        .then(|| draws.into_iter().map(|draw| draw.id).collect())
}

pub(super) fn choices(catalog: &SkillEffectCatalog, round: &FightRound) -> Vec<RandomBuffChoice> {
    type Observations =
        std::collections::HashMap<(ConfiguredRuleKey, i64), Option<RandomBuffChoice>>;
    fn visit(step: &FightStep, catalog: &SkillEffectCatalog, observations: &mut Observations) {
        if let Some((effect, source)) = step
            .act_id
            .and_then(|id| catalog.get(id))
            .zip(step.from_id.filter(|uid| *uid != 0))
            .filter(|_| step.act_type == Some(ActType::Skill as i32))
        {
            for slot in &effect.slots {
                let behavior = &slot.behavior;
                let Some(pool) = behavior::random_arsenal_pool(behavior) else {
                    continue;
                };
                let Some(definition) = behavior::registry::find(behavior) else {
                    continue;
                };
                let rule =
                    ConfiguredRuleKey::new(behavior.effect_id, behavior.slot, definition.key);
                let count = usize::try_from(behavior.arg(1).unwrap_or_default())
                    .unwrap_or_default()
                    .min(pool.len());
                let choice = || {
                    let selected = observed(step, source, &pool, count)?;
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
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            visit(child, catalog, observations);
        }
    }

    let mut observations = Observations::new();
    for step in &round.fight_step {
        visit(step, catalog, &mut observations);
    }
    observations.into_values().flatten().collect()
}

#[cfg(test)]
mod tests;
