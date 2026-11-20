use battle::tooling::{
    replay::{ConfiguredRuleKey, RandomBuffChoice},
    scan::{SkillEffectCatalog, behavior},
};
use sonettobuf::{FightRound, FightStep, effect_type_enum::EffectType};

pub(super) fn choices(catalog: &SkillEffectCatalog, round: &FightRound) -> Vec<RandomBuffChoice> {
    fn visit(catalog: &SkillEffectCatalog, step: &FightStep, choices: &mut Vec<RandomBuffChoice>) {
        if let Some((effect, source_uid)) = step
            .act_id
            .and_then(|id| catalog.get(id))
            .zip(step.from_id.filter(|uid| *uid != 0))
            && effect
                .slots
                .iter()
                .all(|slot| behavior::registry::find(&slot.behavior).is_some())
        {
            let mut grants = Vec::<(i64, Vec<i32>)>::new();
            for record in &step.act_effect {
                if record.effect_type != Some(EffectType::Buffadd as i32) {
                    continue;
                }
                let Some((target_uid, buff)) = record.target_id.zip(record.buff.as_ref()) else {
                    continue;
                };
                let Some(buff_id) = buff.buff_id.filter(|_| buff.from_uid == Some(source_uid))
                else {
                    continue;
                };
                if let Some((_, ids)) = grants.iter_mut().find(|(uid, _)| *uid == target_uid) {
                    ids.push(buff_id);
                } else {
                    grants.push((target_uid, vec![buff_id]));
                }
            }
            for (target_uid, ids) in grants {
                let mut candidates = effect.slots.iter().filter_map(|slot| {
                    let rule = &slot.behavior;
                    let definition = behavior::registry::find(rule)?;
                    if definition.key.opcode != 20021
                        || definition.key.type_name != "AddBuffRanId"
                        || !behavior::is_supported(rule)
                    {
                        return None;
                    }
                    let pool = behavior::random_buff_pool(rule)?;
                    let selected = ids
                        .iter()
                        .copied()
                        .filter(|id| pool.contains(id))
                        .collect::<Vec<_>>();
                    let competing_draws = effect
                        .slots
                        .iter()
                        .filter(|other| !std::ptr::eq(*other, slot))
                        .map(|other| {
                            let other_rule = &other.behavior;
                            let other_definition = behavior::registry::find(other_rule).unwrap();
                            if let Some(other_pool) = behavior::random_buff_pool(other_rule) {
                                if !other_pool.iter().any(|id| selected.contains(id)) {
                                    return 0;
                                }
                                if other_definition.key != definition.key
                                    || other_pool != pool
                                    || other_rule.slot <= rule.slot
                                {
                                    return selected.len();
                                }
                                return other_rule.arg(1).unwrap_or_default().max(0) as usize;
                            }
                            usize::from(
                                (other_definition.references)(other_rule)
                                    .buffs
                                    .iter()
                                    .any(|id| selected.contains(id)),
                            ) * selected.len()
                        })
                        .sum::<usize>();
                    (rule.arg(1) == Some(selected.len() as i32)
                        && !selected.is_empty()
                        && competing_draws < selected.len()
                        && selected
                            .iter()
                            .enumerate()
                            .all(|(index, id)| !selected[..index].contains(id)))
                    .then_some(RandomBuffChoice {
                        rule: ConfiguredRuleKey::new(rule.effect_id, rule.slot, definition.key),
                        source_uid,
                        target_uid,
                        buff_ids: selected,
                    })
                });
                if let Some(choice) = candidates.next()
                    && candidates.next().is_none()
                {
                    choices.push(choice);
                }
            }
        }
        for child in step
            .act_effect
            .iter()
            .filter_map(|effect| effect.fight_step.as_ref())
        {
            visit(catalog, child, choices);
        }
    }

    let mut choices = Vec::new();
    for step in &round.fight_step {
        visit(catalog, step, &mut choices);
    }
    choices
}

#[cfg(test)]
mod tests;
