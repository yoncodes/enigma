use crate::engine::{
    entity::passive::PassiveSourceKind,
    manager::BattleManagers,
    skill::{
        condition::{
            ParsedConditionKind,
            buff::BuffConditionMode,
            registry::{self, ConditionReadTiming},
        },
        effect::{SkillEffectCatalog, SkillEffectSlot},
        target::TargetPool,
    },
};

#[derive(Default)]
pub(crate) struct OpeningTargetBuffs {
    present: Vec<(i64, i32)>,
}

impl OpeningTargetBuffs {
    pub(crate) fn capture(
        source_uid: i64,
        effects: &SkillEffectCatalog,
        managers: &BattleManagers,
        pool: &TargetPool,
    ) -> Self {
        let Some(source) = pool.entity(source_uid) else {
            return Self::default();
        };
        let mut buff_ids = managers
            .entity
            .passive_override(source_uid)
            .unwrap_or(&source.passive_skills)
            .iter()
            .filter(|skill| {
                managers.entity.passive_source(source_uid, **skill) == PassiveSourceKind::Psychube
            })
            .filter_map(|skill| effects.get(*skill))
            .flat_map(|effect| &effect.slots)
            .filter_map(target_buff_ids)
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        if buff_ids.is_empty() {
            return Self::default();
        }
        buff_ids.sort_unstable();
        buff_ids.dedup();
        let mut present = Vec::new();
        for enemy in pool.enemies(source_uid, false) {
            present.extend(
                buff_ids
                    .iter()
                    .filter(|id| managers.buff.has_active_buff_id_or_type(enemy.uid, **id))
                    .map(|id| (enemy.uid, *id)),
            );
        }
        Self { present }
    }

    pub(super) fn matches(&self, slot: &SkillEffectSlot, targets: &[i64]) -> Option<bool> {
        let buff_ids = target_buff_ids(slot)?;
        Some(targets.iter().any(|uid| {
            buff_ids
                .iter()
                .any(|id| self.present.contains(&(*uid, *id)))
        }))
    }
}

fn target_buff_ids(slot: &SkillEffectSlot) -> Option<&[i32]> {
    if !slot.target_from_condition || slot.condition_target.code != 0 {
        return None;
    }
    let [condition] = slot.conditions.as_slice() else {
        return None;
    };
    let definition = registry::find_key(condition.opcode, &condition.type_name)?;
    if definition.read_timing != ConditionReadTiming::PsychubeTargetAtActionStart {
        return None;
    }
    match &condition.kind {
        ParsedConditionKind::BuffId {
            mode: BuffConditionMode::Present,
            buff_ids,
        } => Some(buff_ids),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
