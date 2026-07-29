use crate::engine::{
    manager::{BattleManagers, buff::BuffStatus, emitter},
    runtime::determinism::RoundDeterminism,
    skill::target::{TargetContext, TargetEntity, TargetPool, request::TargetRequest},
};

pub struct TargetResolver;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TargetRule {
    Logic,
    Fixed(i64),
    EventSubject,
    Source,
    Allies,
    AssistBoss,
    BossAllies,
    MainAllies,
    OtherAllies,
    RandomAllyByRng,
    RandomOtherAllyByRng,
    LowestHpPercentageAlly,
    LowestHpAlly,
    HighestAttackAlly,
    LowestExPointAlly,
    HighestExPointAlly,
    AllyPosition(i32),
    AdjacentAllies,
    AdjacentAlly(i32),
    RelativeAllies { before: bool, include_source: bool },
    OddPositionAllies,
    AlliesWithBattleTag,
    AlliesWithMonsterLabel(i32),
    Runtime,
    SynchronizationTarget,
    SingleOrRandomEnemy,
    RandomEnemyByRng,
    EnemiesIncludingSpecial,
    PriorityBossEnemy,
    LowestHpPercentageEnemy,
    HighestAttackEnemy,
    HighestHpEnemy,
    SecondaryEnemies,
    EnemyMostShell,
    EnemyMostQueuedAttacks,
    EnemyPosition(i32),
    RandomEnemy,
    EnemyWithSourceModelLabel,
    EnemyWithConfiguredBuffType,
    Enemies,
    EnemyWithBuffAct,
    EnemiesWithMonsterLabel(i32),
    AlliesWithStatus,
    AlliesWithoutStatus,
    SelectedTarget,
    PriorityEnemyWithMonsterLabel(i32),
}

pub fn is_random_other_ally(target_code: i32) -> bool {
    matches!(
        target_rule(target_code),
        Some(TargetRule::RandomOtherAllyByRng)
    )
}

impl TargetResolver {
    pub fn retarget_stale_explicit(
        source_uid: i64,
        target_uid: i64,
        pool: &TargetPool,
        managers: &BattleManagers,
    ) -> Option<i64> {
        let current = pool.runtime_view(managers);
        if current.entity(target_uid).is_some() {
            return Some(target_uid);
        }

        let source_team = pool.team_type(source_uid)?;
        let target_team = pool.team_type(target_uid)?;
        let candidates = if source_team == target_team {
            current.allies(source_uid)
        } else {
            current.enemies(source_uid, false)
        };
        candidates.first().map(|entity| entity.uid)
    }

    pub fn resolve(
        request: &TargetRequest,
        skill_id: i32,
        source_uid: i64,
        pool: &TargetPool,
        determinism: &mut RoundDeterminism,
    ) -> Vec<i64> {
        Self::resolve_with_context(
            request,
            skill_id,
            source_uid,
            pool,
            determinism,
            TargetContext::default(),
        )
    }

    pub fn resolve_with_context(
        request: &TargetRequest,
        skill_id: i32,
        source_uid: i64,
        pool: &TargetPool,
        determinism: &mut RoundDeterminism,
        context: TargetContext,
    ) -> Vec<i64> {
        Self::resolve_with_managers_and_context(
            request,
            skill_id,
            source_uid,
            pool,
            determinism,
            None,
            context,
        )
    }

    pub fn resolve_with_managers_and_context(
        request: &TargetRequest,
        skill_id: i32,
        source_uid: i64,
        pool: &TargetPool,
        determinism: &mut RoundDeterminism,
        managers: Option<&BattleManagers>,
        context: TargetContext,
    ) -> Vec<i64> {
        let rule = target_rule(request.code);
        if rule.is_none() {
            eprintln!(
                "unsupported target: code={} raw={:?} skill={} source={}",
                request.code, request.raw, skill_id, source_uid,
            );
        }
        let Some(rule) = rule else {
            return Vec::new();
        };
        if matches!(
            rule,
            TargetRule::RandomAllyByRng
                | TargetRule::RandomOtherAllyByRng
                | TargetRule::SingleOrRandomEnemy
                | TargetRule::RandomEnemyByRng
                | TargetRule::RandomEnemy
        ) && let Some(targets) =
            determinism.take_skill_targets(skill_id, source_uid, request.code)
        {
            let candidates = match rule {
                TargetRule::RandomAllyByRng => pool.allies(source_uid).to_vec(),
                TargetRule::RandomOtherAllyByRng => pool
                    .allies(source_uid)
                    .iter()
                    .filter(|entity| entity.uid != source_uid)
                    .cloned()
                    .collect(),
                _ => pool.enemies(source_uid, false).to_vec(),
            };
            if !targets.is_empty()
                && targets
                    .iter()
                    .all(|uid| candidates.iter().any(|candidate| candidate.uid == *uid))
            {
                return targets;
            }
            eprintln!(
                "invalid captured target: code={} skill={} source={} captured={targets:?}",
                request.code, skill_id, source_uid,
            );
        }

        let mut targets = match rule {
            TargetRule::Logic => {
                logic_target(source_uid, skill_id, pool, determinism, managers, context)
            }
            TargetRule::Fixed(uid) => vec![uid],
            TargetRule::EventSubject => runtime_target(context),
            TargetRule::Source => vec![source_uid],
            TargetRule::Allies => {
                if crate::engine::fight::rules::is_side_uid(source_uid) {
                    vec![source_uid]
                } else {
                    uids(pool.allies(source_uid))
                }
            }
            TargetRule::AssistBoss => pool.assist_boss(source_uid),
            TargetRule::BossAllies => pool.boss_allies(source_uid),
            TargetRule::MainAllies => uids(pool.main_allies(source_uid)),
            TargetRule::OtherAllies => other_allies(pool, source_uid, context),
            TargetRule::RandomAllyByRng => random_ally_by_rng(pool.allies(source_uid), determinism),
            TargetRule::RandomOtherAllyByRng => random_ally_by_rng(
                &pool
                    .allies(source_uid)
                    .iter()
                    .filter(|entity| entity.uid != source_uid)
                    .cloned()
                    .collect::<Vec<_>>(),
                determinism,
            ),
            TargetRule::LowestHpPercentageAlly => lowest_hp_percentage(pool.allies(source_uid)),
            TargetRule::LowestHpAlly => lowest_hp(pool.allies(source_uid)),
            TargetRule::HighestAttackAlly => highest_attack(pool.allies(source_uid), managers),
            TargetRule::LowestExPointAlly => lowest_ex_point(pool.allies(source_uid), source_uid),
            TargetRule::HighestExPointAlly => {
                highest_ex_point(pool.main_allies(source_uid), source_uid, managers)
            }
            TargetRule::AllyPosition(position) => at_position(pool.allies(source_uid), position),
            TargetRule::AdjacentAllies => adjacent_allies(pool.allies(source_uid), source_uid),
            TargetRule::AdjacentAlly(offset) => {
                adjacent_ally(pool.allies(source_uid), source_uid, offset)
            }
            TargetRule::RelativeAllies {
                before,
                include_source,
            } => {
                allies_before_or_after(pool.allies(source_uid), source_uid, before, include_source)
            }
            TargetRule::OddPositionAllies => pool
                .allies(source_uid)
                .iter()
                .filter(|entity| matches!(entity.position, 1 | 3))
                .map(|entity| entity.uid)
                .collect(),
            TargetRule::AlliesWithBattleTag => request
                .raw
                .first()
                .copied()
                .map(|tag| {
                    pool.allies(source_uid)
                        .iter()
                        .filter(|entity| entity.battle_tags.contains(&tag))
                        .map(|entity| entity.uid)
                        .collect()
                })
                .unwrap_or_default(),
            TargetRule::AlliesWithMonsterLabel(label) => {
                entities_with_monster_label(pool.allies(source_uid), label)
            }
            TargetRule::Runtime => runtime_target(context),
            TargetRule::SelectedTarget => selected_target(pool, source_uid, context),
            TargetRule::SynchronizationTarget => runtime_target(context),
            TargetRule::SingleOrRandomEnemy => {
                single_and_random_enemy(pool.enemies(source_uid, false), context)
            }
            TargetRule::RandomEnemyByRng => {
                random_enemy_by_rng(pool.enemies(source_uid, false), context, determinism)
            }
            TargetRule::EnemiesIncludingSpecial => uids(pool.enemies(source_uid, true)),
            TargetRule::PriorityBossEnemy => pool.first_boss_enemy(source_uid),
            TargetRule::LowestHpPercentageEnemy => {
                lowest_hp_percentage(pool.enemies(source_uid, false))
            }
            TargetRule::HighestAttackEnemy => {
                highest_attack(pool.enemies(source_uid, false), managers)
            }
            TargetRule::HighestHpEnemy => highest_hp(pool.enemies(source_uid, false), managers),
            TargetRule::SecondaryEnemies => {
                secondary_targets(pool.enemies(source_uid, false), context)
            }
            TargetRule::EnemyMostShell => enemy_with_most_shell(
                pool.enemies(source_uid, false),
                source_uid,
                managers,
                context,
            ),
            TargetRule::EnemyMostQueuedAttacks => {
                enemy_with_most_queued_attacks(pool.enemies(source_uid, false), managers)
            }
            TargetRule::EnemyPosition(position) => {
                at_position(pool.enemies(source_uid, false), position)
            }
            TargetRule::RandomEnemy => random_enemy(pool.enemies(source_uid, false), context),
            TargetRule::EnemyWithSourceModelLabel => enemy_with_source_model_label(
                pool.enemies(source_uid, false),
                pool.entity(source_uid),
            ),
            TargetRule::EnemyWithConfiguredBuffType => request
                .raw
                .first()
                .copied()
                .map(|type_id| enemy_with_buff_type(pool.enemies(source_uid, false), type_id))
                .unwrap_or_default(),
            TargetRule::Enemies => uids(pool.enemies(source_uid, false)),
            TargetRule::EnemyWithBuffAct => enemy_with_buff_act_kind(
                pool.enemies(source_uid, false),
                crate::engine::skill::buff_act::registry::BuffActKind::TargetingTag,
                context,
            ),
            TargetRule::EnemiesWithMonsterLabel(label) => {
                entities_with_monster_label(pool.enemies(source_uid, false), label)
            }
            TargetRule::PriorityEnemyWithMonsterLabel(label) => {
                priority_enemy_with_monster_label(pool.enemies(source_uid, false), label, context)
            }
            TargetRule::AlliesWithStatus => allies_by_status(pool, source_uid, request, true),
            TargetRule::AlliesWithoutStatus => allies_by_status(pool, source_uid, request, false),
        };
        if context.active_skill_is_attack
            && targets.len() > 1
            && let Some(index) = targets
                .iter()
                .position(|uid| *uid == context.runtime_target_uid)
        {
            let primary = targets.remove(index);
            targets.insert(0, primary);
        }
        if let Some(captured) = determinism.take_skill_targets(skill_id, source_uid, request.code)
            && captured != targets
        {
            let available = pool
                .enemies(source_uid, true)
                .iter()
                .map(|entity| (entity.uid, entity.current_hp))
                .collect::<Vec<_>>();
            eprintln!(
                "target drift: code={} skill={} source={} runtime={} logic={} attack={} resolved={targets:?} captured={captured:?} available={available:?}",
                request.code,
                skill_id,
                source_uid,
                context.runtime_target_uid,
                context.logic_target,
                context.active_skill_is_attack,
            );
        }
        targets
    }

    pub fn resolve_action_targets(
        request: &TargetRequest,
        skill_id: i32,
        source_uid: i64,
        pool: &TargetPool,
        determinism: &mut RoundDeterminism,
        managers: Option<&BattleManagers>,
        context: TargetContext,
    ) -> Vec<i64> {
        let targets = Self::resolve_with_managers_and_context(
            request,
            skill_id,
            source_uid,
            pool,
            determinism,
            managers,
            context,
        );
        let targets = redirect_provoked_single_target(targets, source_uid, pool, context);
        let targets = redirect_taunted_single_target(targets, source_uid, pool, context);
        redirect_mock_taunted_single_target(targets, source_uid, pool, context)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn resolve_primary_candidates(
        request: &TargetRequest,
        skill_id: i32,
        source_uid: i64,
        pool: &TargetPool,
        determinism: &RoundDeterminism,
        managers: Option<&BattleManagers>,
        context: TargetContext,
    ) -> Vec<i64> {
        let targets_enemy = targets_enemy(request.code).or_else(|| {
            matches!(target_rule(request.code), Some(TargetRule::SelectedTarget)).then_some(
                context.active_skill_is_attack
                    || context.active_skill_effect_tag
                        == crate::engine::skill::effect::catalog::SkillEffectTag::Debuff as i32,
            )
        });
        let mut resolved = Vec::new();
        for runtime_target_uid in pool.entities().filter_map(|entity| {
            let source_team = pool.team_type(source_uid)?;
            let target_team = pool.team_type(entity.uid)?;
            targets_enemy
                .is_none_or(|enemy| (source_team != target_team) == enemy)
                .then_some(entity.uid)
        }) {
            let mut determinism = determinism.clone();
            let targets = Self::resolve_action_targets(
                request,
                skill_id,
                source_uid,
                pool,
                &mut determinism,
                managers,
                TargetContext {
                    runtime_target_uid,
                    ..context
                },
            );
            if let Some(target_uid) = targets
                .into_iter()
                .find(|target_uid| pool.entity(*target_uid).is_some())
                && !resolved.contains(&target_uid)
            {
                resolved.push(target_uid);
            }
        }
        resolved
    }
}

fn redirect_taunted_single_target(
    targets: Vec<i64>,
    source_uid: i64,
    pool: &TargetPool,
    context: TargetContext,
) -> Vec<i64> {
    if !context.active_skill_is_attack || targets.len() != 1 {
        return targets;
    }
    let taunted = pool
        .enemies(source_uid, false)
        .iter()
        .filter(|entity| {
            entity.has_buff_act_kind(crate::engine::skill::buff_act::registry::BuffActKind::Taunt)
        })
        .collect::<Vec<_>>();
    if taunted.is_empty()
        || taunted
            .iter()
            .any(|entity| Some(entity.uid) == targets.first().copied())
    {
        return targets;
    }
    vec![taunted[0].uid]
}

fn redirect_mock_taunted_single_target(
    targets: Vec<i64>,
    source_uid: i64,
    pool: &TargetPool,
    context: TargetContext,
) -> Vec<i64> {
    if !context.active_skill_is_attack || targets.len() != 1 {
        return targets;
    }
    let priority_targets = pool
        .enemies(source_uid, false)
        .iter()
        .filter(|entity| {
            entity
                .has_buff_act_kind(crate::engine::skill::buff_act::registry::BuffActKind::MockTaunt)
        })
        .collect::<Vec<_>>();
    if priority_targets.is_empty()
        || priority_targets
            .iter()
            .any(|entity| Some(entity.uid) == targets.first().copied())
    {
        return targets;
    }
    vec![priority_targets[0].uid]
}

fn redirect_provoked_single_target(
    targets: Vec<i64>,
    source_uid: i64,
    pool: &TargetPool,
    context: TargetContext,
) -> Vec<i64> {
    if !context.active_skill_is_attack || targets.len() != 1 {
        return targets;
    }
    let Some(provoke_source) = pool.entity(source_uid).and_then(|source| {
        source.buff_source_for_kind(crate::engine::skill::buff_act::registry::BuffActKind::Provoke)
    }) else {
        return targets;
    };
    if pool
        .enemies(source_uid, false)
        .iter()
        .any(|enemy| enemy.uid == provoke_source)
    {
        vec![provoke_source]
    } else {
        targets
    }
}

pub fn is_mapped_target_code(code: i32) -> bool {
    target_rule(code).is_some()
}

pub fn targets_enemy(code: i32) -> Option<bool> {
    match target_rule(code)? {
        TargetRule::Source
        | TargetRule::Allies
        | TargetRule::AssistBoss
        | TargetRule::BossAllies
        | TargetRule::MainAllies
        | TargetRule::OtherAllies
        | TargetRule::RandomAllyByRng
        | TargetRule::RandomOtherAllyByRng
        | TargetRule::LowestHpPercentageAlly
        | TargetRule::LowestHpAlly
        | TargetRule::HighestAttackAlly
        | TargetRule::LowestExPointAlly
        | TargetRule::HighestExPointAlly
        | TargetRule::AllyPosition(_)
        | TargetRule::AdjacentAllies
        | TargetRule::AdjacentAlly(_)
        | TargetRule::RelativeAllies { .. }
        | TargetRule::OddPositionAllies
        | TargetRule::AlliesWithBattleTag
        | TargetRule::AlliesWithMonsterLabel(_)
        | TargetRule::AlliesWithStatus
        | TargetRule::AlliesWithoutStatus => Some(false),
        TargetRule::SingleOrRandomEnemy
        | TargetRule::RandomEnemyByRng
        | TargetRule::EnemiesIncludingSpecial
        | TargetRule::PriorityBossEnemy
        | TargetRule::LowestHpPercentageEnemy
        | TargetRule::HighestAttackEnemy
        | TargetRule::HighestHpEnemy
        | TargetRule::SecondaryEnemies
        | TargetRule::EnemyMostShell
        | TargetRule::EnemyMostQueuedAttacks
        | TargetRule::EnemyPosition(_)
        | TargetRule::RandomEnemy
        | TargetRule::EnemyWithSourceModelLabel
        | TargetRule::EnemyWithConfiguredBuffType
        | TargetRule::Enemies
        | TargetRule::EnemyWithBuffAct
        | TargetRule::PriorityEnemyWithMonsterLabel(_)
        | TargetRule::EnemiesWithMonsterLabel(_) => Some(true),
        TargetRule::Logic
        | TargetRule::Fixed(_)
        | TargetRule::EventSubject
        | TargetRule::Runtime
        | TargetRule::SelectedTarget
        | TargetRule::SynchronizationTarget => None,
    }
}

fn target_rule(code: i32) -> Option<TargetRule> {
    Some(match code {
        0 => TargetRule::Logic,
        6 => TargetRule::Fixed(emitter::UID),
        8 => TargetRule::EventSubject,
        super::request::SOURCE_TARGET_CODE => TargetRule::Source,
        4 | 5 | 104 | 105 | 130 => TargetRule::Allies,
        1005 => TargetRule::BossAllies,
        101 => TargetRule::MainAllies,
        102 => TargetRule::OtherAllies,
        106 => TargetRule::RandomAllyByRng,
        131 => TargetRule::RandomOtherAllyByRng,
        107 => TargetRule::LowestHpPercentageAlly,
        108 => TargetRule::HighestAttackAlly,
        109 => TargetRule::LowestHpAlly,
        111 => TargetRule::LowestExPointAlly,
        112 => TargetRule::HighestExPointAlly,
        113..=116 => TargetRule::AllyPosition(code - 112),
        117 => TargetRule::AdjacentAllies,
        118 => TargetRule::AdjacentAlly(1),
        119 | 128 => TargetRule::AdjacentAlly(-1),
        129 => TargetRule::AdjacentAlly(1),
        120 => TargetRule::RelativeAllies {
            before: true,
            include_source: true,
        },
        121 => TargetRule::RelativeAllies {
            before: true,
            include_source: false,
        },
        122 => TargetRule::RelativeAllies {
            before: false,
            include_source: true,
        },
        123 => TargetRule::RelativeAllies {
            before: false,
            include_source: false,
        },
        124 => TargetRule::AllyPosition(1),
        127 => TargetRule::OddPositionAllies,
        132 => TargetRule::AlliesWithBattleTag,
        1007 => TargetRule::AlliesWithMonsterLabel(7),
        1008 => TargetRule::AlliesWithMonsterLabel(8),
        1009 => TargetRule::AlliesWithMonsterLabel(9),
        1010 => TargetRule::AlliesWithMonsterLabel(10),
        1 => TargetRule::SelectedTarget,
        203 | 204 | 205 | 233 | 303 | 1001 | 1002 => TargetRule::Runtime,
        7 => TargetRule::SynchronizationTarget,
        201 => TargetRule::SingleOrRandomEnemy,
        206 => TargetRule::RandomEnemyByRng,
        202 => TargetRule::EnemiesIncludingSpecial,
        221 => TargetRule::PriorityBossEnemy,
        210 => TargetRule::LowestHpPercentageEnemy,
        207 => TargetRule::HighestAttackEnemy,
        208 => TargetRule::HighestHpEnemy,
        216 => TargetRule::SecondaryEnemies,
        235 => TargetRule::EnemyMostShell,
        244 => TargetRule::EnemyMostQueuedAttacks,
        222..=225 => TargetRule::EnemyPosition(code - 221),
        231 => TargetRule::EnemyPosition(2),
        232 => TargetRule::EnemyPosition(7),
        226..=229 => TargetRule::EnemyPosition(code - 225),
        230 => TargetRule::PriorityEnemyWithMonsterLabel(3097),
        234 => TargetRule::AssistBoss,
        236 => TargetRule::RandomEnemy,
        245 => TargetRule::EnemyWithSourceModelLabel,
        247 => TargetRule::EnemyWithConfiguredBuffType,
        249 => TargetRule::AlliesWithStatus,
        250 => TargetRule::AlliesWithoutStatus,
        301 | 302 => TargetRule::Enemies,
        307 => TargetRule::EnemyWithBuffAct,
        4101 => TargetRule::EnemiesWithMonsterLabel(101),
        _ => return None,
    })
}

fn allies_by_status(
    pool: &TargetPool,
    source_uid: i64,
    request: &TargetRequest,
    present: bool,
) -> Vec<i64> {
    let status = request
        .raw
        .first()
        .copied()
        .map(BuffStatus::from_id)
        .unwrap_or(BuffStatus::Unknown);
    pool.allies(source_uid)
        .iter()
        .filter(|entity| entity.has_buff_status(status) == present)
        .map(|entity| entity.uid)
        .collect()
}

fn logic_target(
    source_uid: i64,
    skill_id: i32,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
    managers: Option<&BattleManagers>,
    context: TargetContext,
) -> Vec<i64> {
    if context.logic_target != 0 {
        let mut nested = context;
        nested.logic_target = 0;
        return TargetResolver::resolve_with_managers_and_context(
            &TargetRequest {
                code: context.logic_target,
                raw: Vec::new(),
            },
            skill_id,
            source_uid,
            pool,
            determinism,
            managers,
            nested,
        );
    }

    if context.runtime_target_uid != 0 {
        return vec![context.runtime_target_uid];
    }

    vec![source_uid]
}

fn runtime_target(context: TargetContext) -> Vec<i64> {
    if context.runtime_target_uid == 0 {
        Vec::new()
    } else {
        vec![context.runtime_target_uid]
    }
}

fn selected_target(pool: &TargetPool, source_uid: i64, context: TargetContext) -> Vec<i64> {
    let selected = runtime_target(context);
    if !selected.is_empty() || !context.active_skill_is_attack {
        return selected;
    }
    pool.enemies(source_uid, false)
        .first()
        .map(|entity| vec![entity.uid])
        .unwrap_or_default()
}

fn uids(entities: &[TargetEntity]) -> Vec<i64> {
    entities.iter().map(|entity| entity.uid).collect()
}

fn other_allies(pool: &TargetPool, source_uid: i64, context: TargetContext) -> Vec<i64> {
    let mut targets = pool
        .allies(source_uid)
        .iter()
        .filter(|entity| entity.uid != source_uid)
        .map(|entity| entity.uid)
        .collect::<Vec<_>>();
    let runtime_target = context.runtime_target_uid;
    if runtime_target != 0
        && runtime_target != source_uid
        && pool
            .team_type(source_uid)
            .is_some_and(|team| pool.team_type(runtime_target) == Some(team))
        && !targets.contains(&runtime_target)
    {
        targets.push(runtime_target);
    }
    targets
}

fn at_position(entities: &[TargetEntity], position: i32) -> Vec<i64> {
    entities
        .iter()
        .filter(|entity| entity.position == position)
        .map(|entity| entity.uid)
        .collect()
}

fn adjacent_ally(entities: &[TargetEntity], source_uid: i64, offset: i32) -> Vec<i64> {
    let target_position = position_of(entities, source_uid) + offset;
    if target_position <= 0 {
        return Vec::new();
    }

    at_position(entities, target_position)
}

fn adjacent_allies(entities: &[TargetEntity], source_uid: i64) -> Vec<i64> {
    let source_position = position_of(entities, source_uid);
    entities
        .iter()
        .filter(|entity| entity.uid != source_uid)
        .filter(|entity| {
            entity.position > 0
                && (entity.position == source_position - 1
                    || entity.position == source_position + 1)
        })
        .map(|entity| entity.uid)
        .collect()
}

fn allies_before_or_after(
    entities: &[TargetEntity],
    source_uid: i64,
    before: bool,
    include_source: bool,
) -> Vec<i64> {
    let source_position = position_of(entities, source_uid);
    let mut output: Vec<i64> = entities
        .iter()
        .filter(|entity| entity.uid != source_uid)
        .filter(|entity| {
            if before {
                entity.position > 0 && entity.position < source_position
            } else {
                entity.position > source_position
            }
        })
        .map(|entity| entity.uid)
        .collect();

    if include_source {
        output.insert(0, source_uid);
    }

    output
}

fn position_of(entities: &[TargetEntity], uid: i64) -> i32 {
    entities
        .iter()
        .find(|entity| entity.uid == uid)
        .map(|entity| entity.position)
        .unwrap_or_default()
}

fn random_ally_by_rng(entities: &[TargetEntity], determinism: &mut RoundDeterminism) -> Vec<i64> {
    determinism
        .lua_random_index(entities.len())
        .map(|index| vec![entities[index].uid])
        .unwrap_or_default()
}

fn lowest_hp_percentage(entities: &[TargetEntity]) -> Vec<i64> {
    entities
        .iter()
        .min_by_key(|entity| {
            (
                entity.current_hp * 10000 / entity.max_hp.max(1),
                entity.position,
                entity.uid,
            )
        })
        .map(|entity| vec![entity.uid])
        .unwrap_or_default()
}

fn lowest_hp(entities: &[TargetEntity]) -> Vec<i64> {
    entities
        .iter()
        .min_by_key(|entity| (entity.current_hp, entity.position, entity.uid))
        .map(|entity| vec![entity.uid])
        .unwrap_or_default()
}

fn highest_attack(entities: &[TargetEntity], managers: Option<&BattleManagers>) -> Vec<i64> {
    entities
        .iter()
        .max_by_key(|entity| {
            let attack = managers
                .map(|managers| {
                    managers
                        .origin_attribute(entity.uid, crate::engine::entity::attr::AttrId::Attack)
                })
                .unwrap_or(entity.attack);
            (
                attack,
                std::cmp::Reverse(entity.position),
                std::cmp::Reverse(entity.uid),
            )
        })
        .map(|entity| vec![entity.uid])
        .unwrap_or_default()
}

fn enemy_with_most_shell(
    enemies: &[TargetEntity],
    source_uid: i64,
    managers: Option<&BattleManagers>,
    context: TargetContext,
) -> Vec<i64> {
    let Some(managers) = managers else {
        return Vec::new();
    };
    let features = managers.buff.active_features(&managers.hp);
    let deployed_buff_id = (context.shell_deployed_buff_id > 0)
        .then_some(context.shell_deployed_buff_id)
        .or_else(|| {
            features.iter().find_map(|feature| {
                (feature.owner_uid == source_uid
                    && feature.values.get(1) == Some(&feature.buff_id)
                    && crate::engine::skill::buff_act::is_kind(
                        feature,
                        crate::engine::skill::buff_act::registry::BuffActKind::ShellProcess,
                    ))
                .then(|| feature.values.get(2).copied())
                .flatten()
            })
        });
    let Some(deployed_buff_id) = deployed_buff_id else {
        return Vec::new();
    };

    enemies
        .iter()
        .filter(|enemy| managers.hp.current(enemy.uid) > 0)
        .filter_map(|enemy| {
            let amount = managers.buff.buff_id_amount(enemy.uid, deployed_buff_id);
            (amount > 0).then_some((enemy, amount))
        })
        .min_by_key(|(enemy, amount)| (-*amount, enemy.position, enemy.uid))
        .map(|(enemy, _)| vec![enemy.uid])
        .unwrap_or_default()
}

fn lowest_ex_point(entities: &[TargetEntity], fallback_uid: i64) -> Vec<i64> {
    let uid = entities
        .iter()
        .min_by_key(|entity| (entity.ex_point, entity.position, entity.uid))
        .map(|entity| entity.uid)
        .unwrap_or(fallback_uid);
    vec![uid]
}

fn highest_ex_point(
    entities: &[TargetEntity],
    fallback_uid: i64,
    managers: Option<&BattleManagers>,
) -> Vec<i64> {
    let uid = entities
        .iter()
        .min_by_key(|entity| {
            (
                -managers
                    .map(|managers| managers.ex_point.get(entity.uid))
                    .unwrap_or(entity.ex_point),
                entity.position,
                entity.uid,
            )
        })
        .map(|entity| entity.uid)
        .unwrap_or(fallback_uid);
    vec![uid]
}

fn highest_hp(entities: &[TargetEntity], managers: Option<&BattleManagers>) -> Vec<i64> {
    entities
        .iter()
        .min_by_key(|entity| {
            let current_hp = managers
                .map(|managers| managers.hp.current(entity.uid))
                .unwrap_or(entity.current_hp);
            let max_hp = managers
                .map(|managers| managers.hp.max(entity.uid))
                .unwrap_or(entity.max_hp)
                .max(1);
            (-(current_hp * 10000 / max_hp), entity.position, entity.uid)
        })
        .map(|entity| vec![entity.uid])
        .unwrap_or_default()
}

fn enemy_with_most_queued_attacks(
    entities: &[TargetEntity],
    managers: Option<&BattleManagers>,
) -> Vec<i64> {
    entities
        .iter()
        .min_by_key(|entity| {
            let count = managers
                .into_iter()
                .flat_map(|managers| managers.card.ai_queue())
                .filter(|card| {
                    card.uid == Some(entity.uid)
                        && card.skill_id.is_some_and(
                            crate::engine::skill::effect::catalog::configured_is_attack,
                        )
                })
                .count();
            (std::cmp::Reverse(count), entity.position, entity.uid)
        })
        .map(|entity| vec![entity.uid])
        .unwrap_or_default()
}

fn single_and_random_enemy(entities: &[TargetEntity], context: TargetContext) -> Vec<i64> {
    let Some(selected) = select_runtime_or_first_enemy(entities, context) else {
        return Vec::new();
    };
    let mut output = vec![selected.uid];
    if let Some(next) = entities
        .iter()
        .skip_while(|entity| entity.uid != selected.uid)
        .skip(1)
        .find(|entity| entity.uid != selected.uid)
    {
        output.push(next.uid);
    }
    output
}

fn random_enemy(entities: &[TargetEntity], context: TargetContext) -> Vec<i64> {
    select_runtime_or_first_enemy(entities, context)
        .map(|entity| vec![entity.uid])
        .unwrap_or_default()
}

fn random_enemy_by_rng(
    entities: &[TargetEntity],
    context: TargetContext,
    determinism: &mut RoundDeterminism,
) -> Vec<i64> {
    if let Some(target) = entities
        .iter()
        .find(|entity| entity.uid == context.runtime_target_uid)
    {
        return vec![target.uid];
    }
    determinism
        .lua_random_index(entities.len())
        .map(|index| vec![entities[index].uid])
        .unwrap_or_default()
}

fn secondary_targets(entities: &[TargetEntity], context: TargetContext) -> Vec<i64> {
    entities
        .iter()
        .filter(|entity| entity.uid != context.runtime_target_uid)
        .map(|entity| entity.uid)
        .collect()
}

fn select_runtime_or_first_enemy(
    entities: &[TargetEntity],
    context: TargetContext,
) -> Option<&TargetEntity> {
    entities
        .iter()
        .find(|entity| entity.uid == context.runtime_target_uid)
        .or_else(|| entities.first())
}

fn enemy_with_source_model_label(
    entities: &[TargetEntity],
    source: Option<&TargetEntity>,
) -> Vec<i64> {
    let Some(label) = source
        .map(|entity| entity.model_id)
        .filter(|label| *label != 0)
    else {
        return Vec::new();
    };
    entities
        .iter()
        .find(|entity| entity.has_monster_label(label))
        .map(|entity| vec![entity.uid])
        .unwrap_or_default()
}

fn enemy_with_buff_type(entities: &[TargetEntity], type_id: i32) -> Vec<i64> {
    entities
        .iter()
        .find(|entity| entity.has_buff_type(type_id))
        .map(|entity| vec![entity.uid])
        .unwrap_or_default()
}

fn enemy_with_buff_act_kind(
    entities: &[TargetEntity],
    kind: crate::engine::skill::buff_act::registry::BuffActKind,
    context: TargetContext,
) -> Vec<i64> {
    let matches: Vec<_> = entities
        .iter()
        .filter(|entity| entity.has_buff_act_kind(kind))
        .collect();

    if matches.is_empty() {
        return Vec::new();
    }

    matches
        .iter()
        .copied()
        .find(|entity| entity.uid == context.runtime_target_uid)
        .or_else(|| matches.first().copied())
        .map(|entity| vec![entity.uid])
        .unwrap_or_default()
}

fn entities_with_monster_label(entities: &[TargetEntity], label: i32) -> Vec<i64> {
    entities
        .iter()
        .filter(|entity| entity.has_monster_label(label))
        .map(|entity| entity.uid)
        .collect()
}

fn priority_enemy_with_monster_label(
    entities: &[TargetEntity],
    label: i32,
    context: TargetContext,
) -> Vec<i64> {
    entities
        .iter()
        .find(|entity| {
            entity.has_monster_label(label)
                && (context.runtime_target_uid == 0 || entity.uid == context.runtime_target_uid)
        })
        .or_else(|| {
            entities
                .iter()
                .find(|entity| entity.has_monster_label(label))
        })
        .or_else(|| {
            entities
                .iter()
                .find(|entity| entity.uid == context.runtime_target_uid)
        })
        .or_else(|| entities.first())
        .map(|entity| vec![entity.uid])
        .unwrap_or_default()
}

#[cfg(test)]
mod test;
