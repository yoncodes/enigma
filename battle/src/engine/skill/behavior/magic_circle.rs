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
    let circle_id = behavior.arg(0)?;
    let row = config::try_get()?.magic_circle.get(circle_id)?;
    let origin = super::command_origin(behavior)?;
    let field = RuleOp::Command(BattleCommand::Field(FieldCommand {
        origin,
        team,
        operation: FieldOperation::DeployIfAbsent {
            definition: FieldDefinition {
                field_id: circle_id,
                duration: row.round,
            },
            create_uid: source_uid,
            initial_level: behavior.arg(1).unwrap_or_default(),
            thresholds: field_thresholds(circle_id, team, managers),
        },
    }));
    if managers.field.get(team).is_some() {
        return Some(vec![field]);
    }
    let (ally_buffs, enemy_buffs) = crate::engine::mechanic::magic_circle::linked_buffs(circle_id);
    let grants = pool
        .allies(source_uid)
        .iter()
        .flat_map(|entity| ally_buffs.iter().map(move |buff_id| (entity.uid, *buff_id)))
        .chain(pool.enemies(source_uid, true).iter().flat_map(|entity| {
            enemy_buffs
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
    let Some(db) = config::try_get() else {
        return Vec::new();
    };
    let mut thresholds = db
        .fight_dnsz
        .iter()
        .filter_map(|threshold| {
            let circle = db.magic_circle.get(threshold.id)?;
            Some(FieldThreshold {
                level: threshold.level,
                progress: threshold.progress,
                definition: FieldDefinition {
                    field_id: threshold.id,
                    duration: circle.round,
                },
            })
        })
        .collect::<Vec<_>>();
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

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        match (behavior.spec.kind, behavior.args.as_slice()) {
            (BehaviorKind::AddMagicCircle, [circle_id, rest @ ..]) => {
                rest.len() <= 1
                    && rest.first().is_none_or(|level| *level >= 0)
                    && config::try_get().is_some_and(|db| db.magic_circle.get(*circle_id).is_some())
            }
            (BehaviorKind::RemoveMagicCircleById, [circle_id]) => {
                config::try_get().is_some_and(|db| db.magic_circle.get(*circle_id).is_some())
            }
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
            _ => None,
        }
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior)
    }
}

fn references(behavior: &ParsedBehavior) -> RuleReferences {
    RuleReferences {
        skills: (behavior.spec.kind == BehaviorKind::AddMagicCircle)
            .then(|| behavior.arg(0))
            .flatten()
            .into_iter()
            .flat_map(self_skills)
            .collect(),
        buffs: Vec::new(),
        models: Vec::new(),
    }
}

pub fn self_skills(circle_id: i32) -> Vec<i32> {
    config::try_get()
        .and_then(|db| db.magic_circle.get(circle_id))
        .map(|row| {
            row.self_skills
                .split(['|', '#'])
                .filter_map(|id| id.trim().parse().ok())
                .filter(|id| *id > 0)
                .collect()
        })
        .unwrap_or_default()
}

pub fn active_self_skills(
    source_uid: i64,
    managers: &BattleManagers,
    pool: &TargetPool,
) -> Vec<i32> {
    pool.team_type(source_uid)
        .and_then(|team| managers.field.get(team))
        .map(|field| self_skills(field.definition.field_id))
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
