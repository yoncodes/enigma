use std::cmp::Reverse;

use sonettobuf::FightDeviceOper;

use crate::engine::{
    manager::BattleManagers,
    runtime::{conduit::admit, determinism::RoundDeterminism},
    skill::{
        effect::SkillEffectCatalog,
        target::{TargetContext, TargetPool},
    },
};

use super::{GroupScore, add_score, skill_score};

pub(in crate::engine::auto_battle) fn select_groups(
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    context: TargetContext,
    operations: &mut [FightDeviceOper],
) {
    let pool = pool.runtime_view(managers);
    for operation in operations {
        let Some(source_uid) = operation.uid else {
            continue;
        };
        if pool.entity(source_uid).is_none() {
            continue;
        }
        let Some(device) = managers
            .conduit
            .devices()
            .find(|device| device.uid == source_uid)
        else {
            continue;
        };
        let current_group = operation
            .index
            .or_else(|| managers.conduit.selected_group(source_uid))
            .unwrap_or(1);
        let group_count = device.skill_groups.len() as i32;
        let mut best: Option<(GroupScore, bool, Reverse<i32>, i32)> = None;
        for group in 1..=group_count {
            let Some(skills) = device.skill_groups.get(group.saturating_sub(1) as usize) else {
                continue;
            };
            let score = skills.iter().fold((0, 0, 0, 0, 0), |total, skill| {
                if catalog.get(skill.skill_id).is_none()
                    || !catalog.issues(skill.skill_id).is_empty()
                {
                    return total;
                }
                let Some(admission) = admit(
                    managers,
                    &pool,
                    catalog,
                    determinism,
                    context,
                    source_uid,
                    *skill,
                    false,
                )
                .filter(|admission| admission.selectable) else {
                    return total;
                };
                add_score(
                    total,
                    skill_score(
                        source_uid,
                        skill.skill_id,
                        admission.cost,
                        managers,
                        &pool,
                        catalog,
                        determinism,
                        context,
                    ),
                )
            });
            if score == (0, 0, 0, 0, 0) {
                continue;
            }
            let candidate = (score, group == current_group, Reverse(group), group);
            if best.as_ref().is_none_or(|best| {
                (candidate.0, candidate.1, candidate.2) > (best.0, best.1, best.2)
            }) {
                best = Some(candidate);
            }
        }
        if let Some((_, _, _, group)) = best {
            operation.index = Some(group);
        }
    }
}
