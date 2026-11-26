use crate::engine::{
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrantChild, array::ArrayBuffOwner},
        emitter,
        field::{FieldCommand, FieldDefinition, FieldOperation, FieldThreshold},
    },
    mechanic::magic_circle_level::MagicCircleLevelCommand,
    runtime::determinism::RoundDeterminism,
    skill::{
        behavior::{
            BehaviorOpContext,
            classify::BehaviorKind,
            registry::{BehaviorHandler, OutputOwner},
        },
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
    let definition = managers.catalog().magic_circle(circle_id)?;
    let origin = super::command_origin(behavior)?;
    let field = RuleOp::Command(BattleCommand::Field(FieldCommand {
        origin,
        team,
        operation: FieldOperation::DeployIfAbsent {
            definition: FieldDefinition {
                field_id: circle_id,
                duration: definition.duration,
            },
            create_uid: source_uid,
            initial_level: behavior.arg(1).unwrap_or_default(),
            thresholds: field_thresholds(circle_id, team, managers),
        },
    }));
    if managers.field.get(team).is_some() {
        return Some(vec![field]);
    }
    let array_owner = managers
        .catalog()
        .uses_array_duration(circle_id)
        .then_some(ArrayBuffOwner {
            team,
            field_id: circle_id,
            caster_uid: source_uid,
        });
    let grant = |target_uid, buff_id| {
        let grant = BuffGrantChild {
            origin,
            source_uid,
            target_uid,
            buff_id,
            amount: None,
            params: None,
            act_info: None,
        };
        let command = match array_owner {
            Some(owner) => BuffCommand::GrantArray { grant, owner },
            None => BuffCommand::GrantChild(grant),
        };
        RuleOp::Command(BattleCommand::Buff(command))
    };
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
        .map(|(target_uid, buff_id)| grant(target_uid, buff_id))
        .collect::<Vec<_>>();
    ops.push(field);
    ops.extend(
        definition
            .linked_buffs
            .into_iter()
            .map(|buff_id| grant(emitter::UID, buff_id)),
    );
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

fn update_wangqi_rule_ops(
    behavior: &ParsedBehavior,
    source_uid: i64,
    team: i32,
    managers: &BattleManagers,
) -> Option<Vec<RuleOp>> {
    if behavior.spec.kind != BehaviorKind::UpdateWangQiMagicCircle
        || behavior.args.as_slice() != [1]
    {
        return None;
    }
    let Some(current) = managers.field.get(team) else {
        return Some(Vec::new());
    };
    let Some(circle) = managers.catalog().magic_circle(current.definition.field_id) else {
        return Some(Vec::new());
    };
    let Some(next) = managers
        .catalog()
        .next_magic_circle_level(current.definition.field_id)
    else {
        return Some(Vec::new());
    };
    Some(vec![RuleOp::Command(BattleCommand::MagicCircleLevel(
        MagicCircleLevelCommand {
            origin: super::command_origin(behavior)?,
            source_uid,
            team,
            expected_field_id: current.definition.field_id,
            replacement: next.definition,
            level: 0,
            removed_buffs: circle.linked_buffs,
            added_buffs: next.linked_buffs,
        },
    ))])
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
            (BehaviorKind::MagicCircleAttr, _) => supports_magic_circle_attr(behavior),
            (BehaviorKind::UpdateWangQiMagicCircle, [1]) => true,
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
            BehaviorKind::MagicCircleAttr => supports_magic_circle_attr(behavior).then(Vec::new),
            BehaviorKind::UpdateWangQiMagicCircle => update_wangqi_rule_ops(
                behavior,
                context.source_uid,
                context.source_team,
                context.managers,
            ),
            _ => None,
        }
    }

    fn output_owner(behavior: &ParsedBehavior, op: &RuleOp, _: usize) -> Option<OutputOwner> {
        let RuleOp::Command(BattleCommand::Buff(
            BuffCommand::GrantChild(grant) | BuffCommand::GrantArray { grant, .. },
        )) = op
        else {
            return None;
        };
        let linked = behavior
            .arg(0)
            .and_then(|circle_id| {
                crate::catalog::BattleCatalog::try_global()?.magic_circle(circle_id)
            })
            .is_some_and(|circle| circle.linked_buffs.contains(&grant.buff_id));
        (grant.target_uid == emitter::UID && linked).then_some(OutputOwner::SetupParent)
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
        summons: Vec::new(),
    }
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
        .map(|definition| definition.self_skills)
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
pub fn emit_attributes(
    attributes: &mut Vec<(AttrId, i32)>,
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
    let Some(owner) = pool.entity(field.owner_uid) else {
        return;
    };
    if pool.source_is_attacker(owner.uid) != pool.source_is_attacker(source_uid) {
        return;
    }

    let mut passive_skills = owner.passive_skills.clone();
    if managers.passives_disabled(owner.uid) {
        passive_skills.clear();
    }
    for link in managers.buff.passive_skill_links_for(owner.uid) {
        if !passive_skills.contains(&link.skill_id) {
            passive_skills.push(link.skill_id);
        }
    }

    for passive_skill in passive_skills {
        let Some(effect) = effects.get(passive_skill) else {
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
                attributes.push((attr_id, *delta));
            }
        }
    }
}

#[cfg(test)]
mod tests;
