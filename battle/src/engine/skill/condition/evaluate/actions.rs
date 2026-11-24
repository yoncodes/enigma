use super::*;

pub(super) fn round_used_minimum_rank(
    minimum_rank: &i32,
    threshold: &i32,
    source_uid: i64,
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
) -> bool {
    managers.is_some_and(|managers| {
        let allies = pool.allies(source_uid);
        managers
            .card
            .played()
            .iter()
            .filter(|played| allies.iter().any(|ally| ally.uid == played.caster_uid))
            .filter(|played| managers.catalog().card_skill_rank(&played.card) >= *minimum_rank)
            .count()
            >= *threshold as usize
    })
}

pub(super) fn can_use_skill(skill_ids: &[i32], context: TargetContext) -> bool {
    // A SkillAction is published only after the cast is accepted. At that
    // boundary the committed active skill is the server-side proof that
    // this configured skill was usable.
    context.active_skill_id != 0 && skill_ids.contains(&context.active_skill_id)
}

pub(super) fn active_use_skill(slot: &i32, pool: &TargetPool, context: TargetContext) -> bool {
    context.direct_skill_body
        && matches!(
            context.active_skill_mode,
            crate::engine::skill::action::SkillExecutionMode::Active
                | crate::engine::skill::action::SkillExecutionMode::DirectBig
                | crate::engine::skill::action::SkillExecutionMode::Device
                | crate::engine::skill::action::SkillExecutionMode::DeviceCard
        )
        && context.extra_skill_kind == 0
        && active_skill_has_real_source(pool, context)
        && (*slot == 0 || context.active_skill_slot == *slot)
}

pub(super) fn active_ally_use_skill(slot: &i32, pool: &TargetPool, context: TargetContext) -> bool {
    context.direct_skill_body
        && matches!(
            context.active_skill_mode,
            crate::engine::skill::action::SkillExecutionMode::Active
                | crate::engine::skill::action::SkillExecutionMode::DirectBig
                | crate::engine::skill::action::SkillExecutionMode::Device
        )
        && context.extra_skill_kind == 0
        && active_skill_has_real_source(pool, context)
        && (*slot == 0 || context.active_skill_slot == *slot)
}

pub(super) fn active_use_skill_by_source(
    slot: &i32,
    condition_targets: &[i64],
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    context.direct_skill_body
        && matches!(
            context.active_skill_mode,
            crate::engine::skill::action::SkillExecutionMode::Active
                | crate::engine::skill::action::SkillExecutionMode::DirectBig
                | crate::engine::skill::action::SkillExecutionMode::Device
        )
        && context.extra_skill_kind == 0
        && active_skill_has_real_source(pool, context)
        && (*slot == 0 || context.active_skill_slot == *slot)
        && condition_targets.contains(&context.active_skill_source_uid)
}

pub(super) fn use_skill_rank(ranks: &[i32], source_uid: i64, context: TargetContext) -> bool {
    context.active_skill_source_uid == source_uid
        && context.active_skill_rank != 0
        && ranks.contains(&context.active_skill_rank)
}

pub(super) fn use_device_skill(pool: &TargetPool, context: TargetContext) -> bool {
    context.active_skill_is_attack
        && matches!(
            context.active_skill_mode,
            crate::engine::skill::action::SkillExecutionMode::Device
                | crate::engine::skill::action::SkillExecutionMode::DeviceCard
        )
        && active_skill_has_real_source(pool, context)
}

pub(super) fn specific_skill(
    group: &i32,
    rank: &i32,
    source_uid: i64,
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    specific_skill_matches(source_uid, *group, *rank, pool, context)
}

pub(super) fn received_specific_skill(
    group: &i32,
    rank: &i32,
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    specific_skill_matches(
        context.active_skill_source_uid,
        *group,
        *rank,
        pool,
        context,
    )
}

pub(super) fn use_ex_skill(
    slot: &i32,
    source_uid: i64,
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    (*slot == 0 || context.active_skill_slot == *slot)
        && card_ultimate(source_uid, pool, managers, context)
}

pub(super) fn after_ultimate(
    source_uid: i64,
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    if context.active_skill_mode == crate::engine::skill::action::SkillExecutionMode::Device {
        managers.is_some_and(|managers| {
            managers
                .conduit
                .is_unique_skill(action_owner(source_uid, context), context.active_skill_id)
        })
    } else {
        card_ultimate(source_uid, pool, managers, context)
    }
}

pub(super) fn teammate_use_ex_skill(
    source_uid: i64,
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    pool.allies(source_uid).iter().any(|ally| {
        ally.uid != source_uid
            && ally.uid == context.active_skill_source_uid
            && ally.ex_skill != 0
            && managers.is_some_and(|managers| {
                crate::engine::mechanic::card::CardMechanic.is_ultimate_skill(
                    managers,
                    context.active_skill_id,
                    ally,
                )
            })
    })
}

pub(super) fn active_skill_rank(
    compare: &ConditionCompare,
    ranks: &[i32],
    context: TargetContext,
) -> bool {
    context.active_skill_rank != 0
        && ranks
            .iter()
            .any(|rank| compare_value(context.active_skill_rank, *compare, *rank))
}
