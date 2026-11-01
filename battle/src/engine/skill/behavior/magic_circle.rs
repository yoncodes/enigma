use crate::engine::{
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrantChild},
        field::{FieldCommand, FieldDefinition, FieldOperation, FieldThreshold},
    },
    runtime::determinism::RoundDeterminism,
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        condition::conditions_match,
        effect::{ParsedBehavior, SkillEffectCatalog},
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
        target::{TargetContext, TargetPool, TargetResolver},
    },
};

pub fn deploy_rule_ops(
    behavior: &ParsedBehavior,
    source_uid: i64,
    team: i32,
    managers: &BattleManagers,
    pool: &TargetPool,
) -> Option<Vec<RuleOp>> {
    if behavior.spec.kind != BehaviorKind::AddMagicCircle {
        return None;
    }
    let mut circle_id = behavior.arg(0)?;
    let family = managers.catalog().emitter_circle_family(circle_id);
    let mut level = family.iter().position(|id| *id == circle_id).unwrap_or_default() as i32;
    let origin = super::command_origin(behavior)?;
    if !family.is_empty() {
        if let Some(current) = managers.field.get(team) {
            if let Some(current_level) = family.iter().position(|id| *id == current.definition.field_id) {
                if level < current_level as i32 { return Some(Vec::new()); }
                let definition = managers.catalog().magic_circle(circle_id)?;
                return Some(vec![RuleOp::Command(BattleCommand::Field(FieldCommand {
                    origin, team, operation: FieldOperation::SetDefinition {
                        definition: FieldDefinition { field_id: circle_id, duration: definition.duration },
                        level,
                    },
                }))]);
            }
        } else if managers.buff.active_features_for_owner(&managers.hp, source_uid).iter()
            .any(|feature| crate::engine::skill::buff_act::is_kind(feature,
                crate::engine::skill::buff_act::registry::BuffActKind::RecordReplaceWqMagicCircle)) {
            if let Some(previous) = managers.field.removed_state(team) {
                if let Some(previous_level) = family.iter().position(|id| *id == previous.definition.field_id) {
                    if previous_level as i32 > level {
                        level = previous_level as i32;
                        circle_id = family[previous_level];
                    }
                }
            }
        }
    }
    let definition = managers.catalog().magic_circle(circle_id)?;
    let field = RuleOp::Command(BattleCommand::Field(FieldCommand {
        origin,
        team,
        operation: FieldOperation::DeployIfAbsent {
            definition: FieldDefinition {
                field_id: circle_id,
                duration: definition.duration,
            },
            create_uid: source_uid,
            initial_level: behavior.arg(1).unwrap_or(level),
            thresholds: field_thresholds(circle_id, team, managers),
        },
    }));
    if managers.field.get(team).is_some() {
        return Some(vec![field]);
    }
    let grants = pool
        .allies(source_uid)
        .iter()
        .flat_map(|entity| {
            definition
                .allied_buffs
                .iter()
                .map(move |buff_id| (entity.uid, *buff_id))
        })
        .chain(pool.enemies(source_uid, true).iter().flat_map(|entity| {
            definition
                .enemy_buffs
                .iter()
                .map(move |buff_id| (entity.uid, *buff_id))
        }));
    let mut ops = grants
        .map(|(target_uid, buff_id)| {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantChild(
                BuffGrantChild {
                    origin,
                    source_uid,
                    target_uid,
                    buff_id,
                    amount: None,
                    params: None,
                    act_info: None,
                },
            )))
        })
        .collect::<Vec<_>>();
    ops.push(field);
    Some(ops)
}

pub(crate) fn upgrade_emitter_circle_rule_ops(
    behavior: &ParsedBehavior,
    team: i32,
    managers: &BattleManagers,
) -> Option<Vec<RuleOp>> {
    let Some(field) = managers.field.get(team) else { return Some(Vec::new()) };
    let family = managers.catalog().emitter_circle_family(field.definition.field_id);
    let Some(level) = family.iter().position(|id| *id == field.definition.field_id) else {
        return Some(Vec::new());
    };
    let upgraded = level.saturating_add(behavior.arg(0)?.max(0) as usize).min(family.len() - 1);
    let circle_id = family[upgraded];
    let definition = managers.catalog().magic_circle(circle_id)?;
    Some(vec![RuleOp::Command(BattleCommand::Field(FieldCommand {
        origin: super::command_origin(behavior)?, team,
        operation: FieldOperation::SetDefinition {
            definition: FieldDefinition { field_id: circle_id, duration: definition.duration },
            level: upgraded as i32,
        },
    }))])
}

pub(crate) fn emitter_circle_round_end_rule_ops(managers: &BattleManagers, team: i32) -> Vec<RuleOp> {
    let Some(field) = managers.field.get(team) else { return Vec::new() };
    let family = managers.catalog().emitter_circle_family(field.definition.field_id);
    if family.is_empty() || field.definition.duration <= 0 { return Vec::new(); }
    let operation = if field.definition.duration == 1 {
        let Some(level) = family.iter().position(|id| *id == field.definition.field_id) else {
            return Vec::new();
        };
        let fallback = level.saturating_sub(1);
        let Some(definition) = managers.catalog().magic_circle(family[fallback]) else {
            return Vec::new();
        };
        FieldOperation::SetDefinition {
            definition: FieldDefinition { field_id: family[fallback], duration: definition.duration },
            level: fallback as i32,
        }
    } else {
        FieldOperation::ChangeDuration { delta: -1 }
    };
    vec![RuleOp::Command(BattleCommand::Field(FieldCommand { origin: field.origin, team, operation }))]
}

pub(crate) fn primitive_urge_circle_round_end_rule_ops(managers: &BattleManagers, team: i32) -> Vec<RuleOp> {
    let Some(field) = managers.field.get(team) else { return Vec::new() };
    // Crimson Dreamscape has no attached entity buffs to clean up. Its
    // owner's configured removal reaction casts the follow-up ultimate.
    if field.definition.field_id != 1_251_001 || field.definition.duration < 0 {
        return Vec::new();
    }
    let operation = if field.definition.duration <= 1 {
        FieldOperation::Remove
    } else {
        FieldOperation::ChangeDuration { delta: -1 }
    };
    vec![RuleOp::Command(BattleCommand::Field(FieldCommand {
        origin: field.origin, team, operation,
    }))]
}

fn remove_rule_ops(
    behavior: &ParsedBehavior,
    team: i32,
    managers: &BattleManagers,
) -> Option<Vec<RuleOp>> {
    if behavior.spec.kind != BehaviorKind::RemoveMagicCircleById {
        return None;
    }
    let circle_id = behavior.arg(0)?;
    let Some(field) = managers.field.get(team) else {
        return Some(Vec::new());
    };
    if field.definition.field_id != circle_id {
        return Some(Vec::new());
    }
    Some(vec![RuleOp::Command(BattleCommand::Field(FieldCommand {
        origin: super::command_origin(behavior)?,
        team,
        operation: FieldOperation::Remove,
    }))])
}

pub(crate) fn field_thresholds(
    circle_id: i32,
    team: i32,
    managers: &BattleManagers,
) -> Vec<FieldThreshold> {
    let mut thresholds = managers.catalog().magic_circle_thresholds();
    thresholds.sort_by_key(|threshold| threshold.level);
    if !thresholds
        .iter()
        .any(|threshold| threshold.definition.field_id == circle_id)
    {
        return Vec::new();
    }
    crate::engine::skill::buff_act::fix_electric_upgrade::resolve_thresholds(
        team,
        &thresholds,
        &managers.buff.active_features(&managers.hp),
    )
}

pub(super) struct Handler;

fn supports_magic_circle_attr(behavior: &ParsedBehavior) -> bool {
    let mut triples = behavior.args.chunks_exact(3);
    !behavior.args.is_empty()
        && triples.remainder().is_empty()
        && triples.all(|triple| {
            let [scope, raw_attr_id, _] = triple else {
                return false;
            };
            matches!(*scope, 1 | 2) && AttrId::from_raw(*raw_attr_id).is_some()
        })
}

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        let configured = |circle_id| {
            crate::catalog::BattleCatalog::try_global()
                .is_some_and(|catalog| catalog.magic_circle(circle_id).is_some())
        };
        match (behavior.spec.kind, behavior.args.as_slice()) {
            (BehaviorKind::AddMagicCircle, [circle_id, rest @ ..]) => {
                rest.len() <= 1
                    && rest.first().is_none_or(|level| *level >= 0)
                    && configured(*circle_id)
            }
            (BehaviorKind::RemoveMagicCircleById, [circle_id]) => configured(*circle_id),
            (BehaviorKind::MagicCircleAddRound, [rounds]) => *rounds != 0,
            (BehaviorKind::MagicCircleAttr, _) => supports_magic_circle_attr(behavior),
            (BehaviorKind::UpdateWangQiMagicCircle, [levels]) => *levels > 0,
            _ => false,
        }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        match behavior.spec.kind {
            BehaviorKind::AddMagicCircle => deploy_rule_ops(
                behavior,
                context.source_uid,
                context.source_team,
                context.managers,
                context.pool,
            ),
            BehaviorKind::RemoveMagicCircleById => {
                remove_rule_ops(behavior, context.source_team, context.managers)
            }
            BehaviorKind::MagicCircleAddRound => {
                let [rounds] = behavior.args.as_slice() else { return None };
                if *rounds == 0 { return None; }
                if context.managers.field.get(context.source_team).is_none() {
                    return Some(Vec::new());
                }
                Some(vec![RuleOp::Command(BattleCommand::Field(FieldCommand {
                    origin: super::command_origin(behavior)?,
                    team: context.source_team,
                    operation: FieldOperation::ChangeDuration { delta: *rounds },
                }))])
            }
            BehaviorKind::MagicCircleAttr => supports_magic_circle_attr(behavior).then(Vec::new),
            BehaviorKind::UpdateWangQiMagicCircle => upgrade_emitter_circle_rule_ops(
                behavior, context.source_team, context.managers,
            ),
            _ => None,
        }
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior)
    }
}

fn references(behavior: &ParsedBehavior) -> RuleReferences {
    let mut references = RuleReferences::default();
    if behavior.spec.kind == BehaviorKind::AddMagicCircle {
        if let Some(circle_id) = behavior.arg(0) {
            references.skills.extend(self_skills(circle_id));
            if let Some(catalog) = crate::catalog::BattleCatalog::try_global() {
                if catalog.magic_circle(circle_id).is_some_and(|circle| {
                    !circle.emitter_skills.is_empty() || !circle.emitter_buffs.is_empty()
                }) {
                    let (skills, buffs) = catalog.emitter_circle_references();
                    references.skills.extend(skills);
                    references.buffs.extend(buffs);
                }
            }
        }
    }
    references
}

pub fn self_skills(circle_id: i32) -> Vec<i32> {
    crate::catalog::BattleCatalog::try_global()
        .and_then(|catalog| catalog.magic_circle(circle_id))
        .map(|definition| definition.self_skills)
        .unwrap_or_default()
}

pub fn active_self_skills(
    source_uid: i64,
    managers: &BattleManagers,
    pool: &TargetPool,
) -> Vec<i32> {
    pool.team_type(source_uid)
        .and_then(|team| managers.field.get(team))
        .and_then(|field| managers.catalog().magic_circle(field.definition.field_id))
        .map(|mut definition| {
            if source_uid == crate::engine::manager::emitter::UID {
                definition.self_skills.extend(definition.emitter_skills);
            }
            definition.self_skills
        })
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
pub fn emit_attack_attributes(
    attack_attributes: &mut Vec<(AttrId, i32)>,
    source_uid: i64,
    active_skill_id: i32,
    effects: &SkillEffectCatalog,
    managers: &BattleManagers,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) {
    let Some(field) = pool
        .team_type(source_uid)
        .and_then(|team| managers.field.get(team))
    else {
        return;
    };
    let Some(owner) = pool.entity(field.create_uid) else {
        return;
    };
    if pool.source_is_attacker(owner.uid) != pool.source_is_attacker(source_uid) {
        return;
    }

    for passive_skill in &owner.passive_skills {
        let Some(effect) = effects.get(*passive_skill) else {
            continue;
        };
        for slot in effect
            .slots
            .iter()
            .filter(|slot| slot.behavior.spec.kind == BehaviorKind::MagicCircleAttr)
        {
            let condition_targets = TargetResolver::resolve_with_managers_and_context(
                &slot.condition_target,
                active_skill_id,
                owner.uid,
                pool,
                determinism,
                Some(managers),
                context,
            );
            if !conditions_match(
                &slot.conditions,
                owner.uid,
                &condition_targets,
                Some(managers),
                pool,
                context,
            ) {
                continue;
            }
            for args in slot.behavior.args.chunks_exact(3) {
                let [scope, raw_attr_id, delta] = args else {
                    continue;
                };
                if *scope == 1 && source_uid != owner.uid {
                    continue;
                }
                if !matches!(*scope, 1 | 2) {
                    continue;
                }
                let Some(attr_id) = AttrId::from_raw(*raw_attr_id) else {
                    continue;
                };
                attack_attributes.push((attr_id, *delta));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_circle_exposes_its_start_phase_skill() {
        crate::test_support::init_config();

        assert_eq!(self_skills(100051), vec![308801821]);
    }

    #[test]
    fn deploying_an_emitter_circle_compiles_its_later_level_rules() {
        crate::test_support::init_config();
        let links = references(&ParsedBehavior::new(50019, "AddMagicCircle", vec![45001]));
        assert!(links.skills.contains(&31414153));
        assert!(links.buffs.contains(&31413604));
        let effects = SkillEffectCatalog::from_roots(
            crate::test_support::game_data(), links.skills, links.buffs,
        );
        assert!(effects.get(31414153).is_some());
    }

    #[test]
    fn magic_circle_attr_accepts_one_or_more_complete_valid_triples() {
        let behavior = |args| ParsedBehavior::new(60076, "MagicCircleAttr", args);

        assert!(Handler::supports(&behavior(vec![2, 214, 120])));
        assert!(Handler::supports(&behavior(vec![1, 301, 80, 2, 214, -120])));
    }

    #[test]
    fn magic_circle_attr_rejects_incomplete_scope_or_attribute_triples() {
        let behavior = |args| ParsedBehavior::new(60076, "MagicCircleAttr", args);

        for args in [vec![2, 214], vec![3, 214, 120], vec![2, 999, 120]] {
            assert!(!Handler::supports(&behavior(args)));
        }
    }

    #[test]
    fn magic_circle_attr_has_a_registry_owner_but_emits_no_runtime_operation() {
        let behavior = ParsedBehavior::new(60076, "MagicCircleAttr", vec![2, 214, 120]);
        let definition = crate::engine::skill::behavior::registry::find(&behavior).unwrap();
        assert!(definition.destination);
        assert!(crate::engine::skill::behavior::is_supported(&behavior));

        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = TargetContext::default();

        assert!(matches!(
            crate::engine::skill::behavior::rule_ops(
                BehaviorOpContext {
                    source_uid: 10,
                    source_team: 1,
                    target_uid: 10,
                    active_skill_id: 0,
                    transfer_count: 1,
                    event: None,
                    managers: &managers,
                    pool: &pool,
                    determinism: &mut determinism,
                    modifiers: &mut modifiers,
                    target: &mut target,
                },
                &behavior,
            ),
            Some(ops) if ops.is_empty()
        ));
    }

    #[test]
    fn add_magic_circle_emits_a_config_derived_deploy_command() {
        crate::test_support::init_config();
        let behavior = ParsedBehavior::new(50019, "AddMagicCircle", vec![30001, 1]);

        let fight = sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);

        assert!(matches!(
            deploy_rule_ops(&behavior, 10, 1, &BattleManagers::default(), &pool).as_deref(),
            Some([RuleOp::Command(BattleCommand::Field(FieldCommand {
                team: 1,
                operation: FieldOperation::DeployIfAbsent {
                    definition: FieldDefinition {
                        field_id: 30001,
                        ..
                    },
                    create_uid: 10,
                    initial_level: 1,
                    thresholds,
                },
                ..
            }))]) if thresholds.iter().any(|threshold| threshold.level == 2 && threshold.progress == 50)
        ));
    }

    #[test]
    fn remove_magic_circle_only_removes_the_matching_active_field() {
        crate::test_support::init_config();
        let origin = crate::engine::manager::buff::CommandOrigin {
            domain: crate::engine::skill::rule::RuleDomain::Behavior,
            key: crate::engine::skill::rule::DefinitionKey::new(50019, "AddMagicCircle"),
        };
        let mut managers = BattleManagers::default();
        managers
            .field
            .execute_command(FieldCommand {
                origin,
                team: 2,
                operation: FieldOperation::DeployIfAbsent {
                    definition: FieldDefinition {
                        field_id: 20009,
                        duration: 4,
                    },
                    create_uid: -3,
                    initial_level: 0,
                    thresholds: Vec::new(),
                },
            })
            .unwrap();

        assert!(
            remove_rule_ops(
                &ParsedBehavior::new(50021, "RemoveMagicCircleById", vec![20008]),
                2,
                &managers,
            )
            .unwrap()
            .is_empty()
        );
        assert!(matches!(
            remove_rule_ops(
                &ParsedBehavior::new(50021, "RemoveMagicCircleById", vec![20009]),
                2,
                &managers,
            )
            .unwrap()
            .as_slice(),
            [RuleOp::Command(BattleCommand::Field(FieldCommand {
                team: 2,
                operation: FieldOperation::Remove,
                ..
            }))]
        ));
    }

    #[test]
    fn blood_domain_deployment_grants_its_configured_ally_buff() {
        crate::test_support::init_config();
        let fight = sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![
                    sonettobuf::FightEntityInfo {
                        uid: Some(10),
                        ..Default::default()
                    },
                    sonettobuf::FightEntityInfo {
                        uid: Some(11),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let behavior = ParsedBehavior::new(50019, "AddMagicCircle", vec![100051]);

        let ops = deploy_rule_ops(&behavior, 10, 1, &BattleManagers::default(), &pool).unwrap();

        assert_eq!(ops.len(), 3);
        assert!(ops[..2].iter().all(|op| matches!(
            op,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantChild(
                BuffGrantChild {
                    buff_id: 308801312,
                    ..
                }
            )))
        )));
        assert!(matches!(ops[2], RuleOp::Command(BattleCommand::Field(_))));
    }
}
