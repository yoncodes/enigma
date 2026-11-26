use super::*;

pub(super) fn hit_damage_percent_more_than(
    threshold_permille: &i32,
    condition_targets: &[i64],
    managers: Option<&BattleManagers>,
    context: TargetContext,
) -> bool {
    managers.is_some_and(|managers| {
        context.hit_resolved_damage > 0
            && context.hit_target_uid != 0
            && condition_targets.contains(&context.hit_target_uid)
            && managers.hp.max(context.hit_target_uid) > 0
            && i64::from(context.hit_resolved_damage) * 1000
                > i64::from(managers.hp.max(context.hit_target_uid))
                    * i64::from(*threshold_permille)
    })
}

pub(super) fn target_shares_caster_career(
    param: &i32,
    source_uid: i64,
    condition_targets: &[i64],
    pool: &TargetPool,
) -> bool {
    let Some(source) = pool.entity(source_uid) else {
        return false;
    };
    let shares = source.career != 0
        && condition_targets
            .iter()
            .filter_map(|uid| pool.entity(*uid))
            .any(|entity| entity.shares_career_with(source));
    shares == (*param == 0)
}

pub(super) fn alive_allied_model_count(
    model_id: &i32,
    minimum: &i32,
    source_uid: i64,
    pool: &TargetPool,
) -> bool {
    pool.main_allies(source_uid)
        .iter()
        .filter(|entity| entity.model_id == *model_id)
        .count() as i32
        >= *minimum
}

pub(super) fn summoned_count(
    summoned_id: &i32,
    required_level: &i32,
    compare: &ConditionCompare,
    count: &i32,
    source_uid: i64,
    managers: Option<&BattleManagers>,
) -> bool {
    managers.is_some_and(|managers| {
        compare_value(
            managers
                .summon
                .count(source_uid, *summoned_id, *required_level),
            *compare,
            *count,
        )
    })
}

pub(super) fn group_summoned_count(
    owner_model_id: &i32,
    required_level: &i32,
    compare: &ConditionCompare,
    count: &i32,
    source_uid: i64,
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
) -> bool {
    managers.is_some_and(|managers| {
        let amount = pool
            .allies(source_uid)
            .iter()
            .filter(|entity| entity.model_id == *owner_model_id)
            .map(|entity| managers.summon.total(entity.uid, *required_level))
            .sum();
        compare_value(amount, *compare, *count)
    })
}

pub(super) fn battle_tag_count(
    tag_id: &i32,
    compare: &ConditionCompare,
    threshold: &i32,
    source_uid: i64,
    pool: &TargetPool,
) -> bool {
    compare_value(
        pool.allies(source_uid)
            .iter()
            .filter(|entity| entity.battle_tags.contains(tag_id))
            .count() as i32,
        *compare,
        *threshold,
    )
}

pub(super) fn target_battle_tag(
    tag_id: &i32,
    condition_targets: &[i64],
    pool: &TargetPool,
) -> bool {
    condition_targets
        .iter()
        .filter_map(|uid| pool.entity(*uid))
        .any(|entity| entity.battle_tags.contains(tag_id))
}

pub(super) fn target_identity(
    mode: &TargetIdentityMode,
    value: &i32,
    source_uid: i64,
    condition_targets: &[i64],
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    target_identity_matches(*mode, *value, source_uid, condition_targets, pool, context)
}

pub(super) fn team_contains_models(model_ids: &[i32], source_uid: i64, pool: &TargetPool) -> bool {
    pool.allies(source_uid).iter().any(|target| {
        model_ids.contains(&target.model_id) || model_ids.contains(&(target.model_id / 10))
    })
}

pub(super) fn team_model_presence(
    model_ids: &[i32],
    present: &bool,
    source_uid: i64,
    pool: &TargetPool,
) -> bool {
    let found = pool.allies(source_uid).iter().any(|target| {
        model_ids.contains(&target.model_id) || model_ids.contains(&(target.model_id / 10))
    });
    found == *present
}

pub(super) fn entity_count_matches(
    scope: EntityCountScope,
    compare: ConditionCompare,
    expected: i32,
    source_uid: i64,
    managers: Option<&BattleManagers>,
    pool: &TargetPool,
) -> bool {
    if pool.entity(source_uid).is_none() {
        return false;
    }

    let source_is_attacker = pool.source_is_attacker(source_uid);
    let alive_count = |entities: &[TargetEntity]| {
        entities
            .iter()
            .filter(|entity| managers.is_none_or(|managers| managers.hp.current(entity.uid) > 0))
            .count()
    };
    let count = match scope {
        EntityCountScope::EnemyTargets
        | EntityCountScope::AliveEnemies
        | EntityCountScope::AliveEnemiesIncludeSp => alive_count(pool.enemies(source_uid, false)),
        EntityCountScope::AliveTeammates => alive_count(pool.allies(source_uid)),
        EntityCountScope::AliveOtherTeammates => pool
            .allies(source_uid)
            .iter()
            .filter(|entity| {
                entity.uid != source_uid
                    && managers.is_none_or(|managers| managers.hp.current(entity.uid) > 0)
            })
            .count(),
        EntityCountScope::AliveTeammatesNoSp => {
            if source_is_attacker {
                alive_count(&pool.attacker_main)
            } else {
                alive_count(&pool.defender_main)
            }
        }
        EntityCountScope::TeamSize | EntityCountScope::HeroCount => {
            if source_is_attacker {
                pool.attacker_main.len()
            } else {
                pool.defender_main.len()
            }
        }
    } as i32;

    compare_value(count, compare, expected)
}

pub(super) fn per_target_career_count(
    source_uid: i64,
    careers: &[i32],
    threshold: i32,
    pool: &TargetPool,
) -> i32 {
    let team = if pool.source_is_attacker(source_uid) {
        &pool.attacker_main
    } else {
        &pool.defender_main
    };
    let count = team
        .iter()
        .filter(|entity| {
            entity.uid != source_uid && careers.iter().any(|career| entity.has_career(*career))
        })
        .count() as i32;
    if threshold > 0 {
        count.min(threshold)
    } else {
        count
    }
}

pub(super) fn team_career_count(source_uid: i64, careers: &[i32], pool: &TargetPool) -> i32 {
    let team = if pool.source_is_attacker(source_uid) {
        &pool.attacker_main
    } else {
        &pool.defender_main
    };
    team.iter()
        .filter(|entity| careers.iter().any(|career| entity.has_career(*career)))
        .count() as i32
}

pub(super) fn per_team_career_count(
    source_uid: i64,
    career: i32,
    opposing_team: bool,
    pool: &TargetPool,
) -> i32 {
    let source_is_attacker = pool.source_is_attacker(source_uid);
    let team = if source_is_attacker != opposing_team {
        &pool.attacker_main
    } else {
        &pool.defender_main
    };
    team.iter()
        .filter(|entity| entity.has_career(career))
        .count() as i32
}

pub(super) fn other_ally_damage_type_count(
    source_uid: i64,
    damage_type: crate::engine::skill::target::EntityDamageType,
    max_count: i32,
    pool: &TargetPool,
) -> i32 {
    pool.allies(source_uid)
        .iter()
        .filter(|entity| entity.uid != source_uid && entity.damage_type == damage_type)
        .count()
        .min(max_count.max(0) as usize) as i32
}

fn target_identity_matches(
    mode: TargetIdentityMode,
    value: i32,
    source_uid: i64,
    condition_targets: &[i64],
    pool: &TargetPool,
    context: TargetContext,
) -> bool {
    if mode == TargetIdentityMode::ActiveSkillSourceModelId {
        return pool
            .entity(context.active_skill_source_uid)
            .is_some_and(|source| source.model_id == value || source.model_id / 10 == value);
    }
    if mode == TargetIdentityMode::ConditionTargetModelId {
        return condition_targets.iter().any(|&uid| {
            pool.entity(uid)
                .is_some_and(|target| target.model_id == value || target.model_id / 10 == value)
        });
    }
    let targets = identity_targets(condition_targets, pool, context);
    match mode {
        TargetIdentityMode::TargetIsSelf => targets.iter().any(|target| target.uid == source_uid),
        TargetIdentityMode::TargetIsAllyNotSelf => targets.iter().any(|target| {
            target.uid != source_uid
                && pool.source_is_attacker(target.uid) == pool.source_is_attacker(source_uid)
        }),
        TargetIdentityMode::TargetModelId => targets
            .iter()
            .any(|target| target.model_id == value || target.model_id / 10 == value),
        TargetIdentityMode::ConditionTargetModelId
        | TargetIdentityMode::ActiveSkillSourceModelId => {
            unreachable!()
        }
        TargetIdentityMode::TargetPosition => targets.iter().any(|target| target.position == value),
    }
}

fn identity_targets<'a>(
    condition_targets: &[i64],
    pool: &'a TargetPool,
    context: TargetContext,
) -> Vec<&'a TargetEntity> {
    if context.runtime_target_uid != 0
        && let Some(target) = pool.entity(context.runtime_target_uid)
    {
        return vec![target];
    }
    let targets: Vec<_> = condition_targets
        .iter()
        .filter_map(|uid| pool.entity(*uid))
        .collect();
    targets
}
