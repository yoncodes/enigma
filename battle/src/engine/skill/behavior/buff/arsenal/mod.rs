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

use super::grant::PoolEntry;

pub(super) struct Bullet {
    pub(super) entry: PoolEntry,
    type_id: i32,
}

pub(super) fn select(
    catalog: BattleCatalog,
    entries: &[PoolEntry],
) -> Result<Option<Vec<Bullet>>, ()> {
    if entries.iter().all(|entry| entry.stacks == 1) {
        return Ok(None);
    }
    let mut bullets = Vec::with_capacity(entries.len());
    for &entry in entries {
        if !catalog.buff_features(entry.id).iter().any(|feature| {
            feature.values.first().is_some_and(|act| {
                registry::kind(*act, &feature.act_type) == Some(BuffActKind::Bullet)
            })
        }) {
            return Ok(None);
        }
        bullets.push(Bullet {
            entry,
            type_id: catalog.buff_type_id(entry.id),
        });
    }
    let mut types = std::collections::HashSet::new();
    if bullets.iter().any(|bullet| {
        bullet.entry.stacks <= 1 || bullet.type_id <= 0 || !types.insert(bullet.type_id)
    }) {
        return Err(());
    }
    Ok(Some(bullets))
}

pub(super) fn grants(
    context: &mut BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
    count: usize,
    bullets: Vec<Bullet>,
) -> Option<Vec<RuleOp>> {
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
        .map(|bullet| bullet.entry.id)
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
        .filter(|observed| {
            let (new, existing) = observed.split_at(count.min(unheld.len()));
            new.iter()
                .all(|id| unheld.iter().any(|bullet| bullet.entry.id == *id))
                && existing
                    .iter()
                    .all(|id| held.iter().any(|bullet| bullet.entry.id == *id))
        })
        .unwrap_or_default()
        .into_iter();
    let mut ops = Vec::with_capacity(count);
    for _ in 0..count {
        let candidates = if unheld.is_empty() {
            &mut held
        } else {
            &mut unheld
        };
        let index = if let Some(id) = observed.next() {
            context.determinism.lua_random_index(candidates.len())?;
            candidates.iter().position(|bullet| bullet.entry.id == id)?
        } else {
            let ids = candidates
                .iter()
                .map(|bullet| bullet.entry.id)
                .collect::<Vec<_>>();
            context
                .determinism
                .take_random_buff(&ids)
                .and_then(|id| ids.iter().position(|candidate| *candidate == id))
                .or_else(|| context.determinism.lua_random_index(candidates.len()))?
        };
        let bullet = candidates.remove(index);
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                origin,
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                buff_id: bullet.entry.id,
                amount: None,
                occurrences: bullet.entry.stacks,
                child_uid_reservations: 0,
            },
        ))));
    }
    Some(ops)
}

#[cfg(test)]
mod tests;
