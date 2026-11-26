use crate::{
    catalog::BattleCatalog,
    engine::{
        manager::buff::{BuffCommand, BuffGrant},
        skill::{
            behavior::BehaviorOpContext,
            buff_act::registry::{self, BuffActKind},
            effect::ParsedBehavior,
            rule::{
                ConfiguredRuleKey,
                output::{BattleCommand, RuleOp},
            },
        },
    },
};

struct Bullet {
    id: i32,
    type_id: i32,
    stacks: u32,
}

fn configured(catalog: BattleCatalog, pool_id: i32) -> Option<Vec<Bullet>> {
    let rows = catalog.buff_feature_rows(pool_id);
    let [pool] = rows.as_slice() else {
        return None;
    };
    let mut types = std::collections::HashSet::new();
    pool.split('#')
        .map(|entry| {
            let (id, stacks) = entry.split_once(',')?;
            let id = id.parse().ok()?;
            let stacks = stacks.parse().ok().filter(|stacks| *stacks > 1)?;
            let type_id = catalog.buff_type_id(id);
            let bullet = catalog.buff_features(id).iter().any(|feature| {
                feature.values.first().is_some_and(|act| {
                    registry::kind(*act, &feature.act_type) == Some(BuffActKind::Bullet)
                })
            });
            (bullet && type_id > 0 && types.insert(type_id)).then_some(Bullet {
                id,
                type_id,
                stacks,
            })
        })
        .collect()
}

pub(super) fn grants(
    context: &mut BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let [pool_id, count] = behavior.args.as_slice() else {
        return None;
    };
    let bullets = configured(context.managers.catalog(), *pool_id)?;
    let count = usize::try_from(*count).ok()?.min(bullets.len());
    let origin = super::command_origin(behavior)?;
    let (mut unheld, mut held): (Vec<_>, Vec<_>) = bullets.into_iter().partition(|bullet| {
        context
            .managers
            .buff
            .buff_type_amount(context.target_uid, bullet.type_id)
            == 0
    });
    let ids = unheld
        .iter()
        .chain(&held)
        .map(|bullet| bullet.id)
        .collect::<Vec<_>>();
    let mut observed = context
        .determinism
        .take_random_buff_pool(
            ConfiguredRuleKey::new(behavior.effect_id, behavior.slot, origin.key),
            context.source_uid,
            context.target_uid,
            count,
            &ids,
        )
        .unwrap_or_default()
        .into_iter();
    let mut ops = Vec::with_capacity(count);
    for _ in 0..count {
        let candidates = if unheld.is_empty() {
            &mut held
        } else {
            &mut unheld
        };
        let candidate_ids = candidates
            .iter()
            .map(|bullet| bullet.id)
            .collect::<Vec<_>>();
        let selected = observed
            .next()
            .filter(|id| candidate_ids.contains(id))
            .or_else(|| context.determinism.take_random_buff(&candidate_ids));
        let index = selected
            .and_then(|id| candidate_ids.iter().position(|candidate| *candidate == id))
            .or_else(|| context.determinism.lua_random_index(candidates.len()))?;
        let bullet = candidates.remove(index);
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin,
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                buff_id: bullet.id,
                amount: None,
                occurrences: bullet.stacks,
                child_uid_reservations: 0,
            },
        ))));
    }
    Some(ops)
}

#[cfg(test)]
mod tests;
