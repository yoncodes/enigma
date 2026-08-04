use crate::engine::{
    entity::skill::Skill,
    manager::{
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand},
        ex_point::{ExPointChange, ExPointCommand},
    },
    runtime::determinism::RoundDeterminism,
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        condition::extra::{ExtraSkillKind, skill_kind_from_is_extra},
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
        target::TargetPool,
    },
};

pub(super) struct Handler;

pub(super) fn supports_consume_power_direct_skill(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [cost, skill_id] if *cost > 0 && *skill_id > 0)
}

pub(super) fn supports_consume_power_skill(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [cost, skill_id] if *cost > 0 && *skill_id > 0)
}

pub(super) fn supports_random_skill(behavior: &ParsedBehavior) -> bool {
    matches!(
        behavior.raw_args.as_slice(),
        [raw] if weighted_skills(raw).is_some()
    )
}

pub(super) fn supports_consume_buff_use_skill(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [buff_id, amount, skill_id, _]
        if *buff_id > 0 && *amount > 0 && *skill_id > 0)
}

pub(super) fn supports_consume_buff_use_skill3(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [buff_id, amount, skill_id]
        if *buff_id > 0 && *amount > 0 && *skill_id > 0)
}

pub(super) fn supports_consume_target_buff_use_skill(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [buff_id, amount, skill_id]
        if *buff_id > 0 && *amount > 0 && *skill_id > 0)
}

pub(super) fn supports_direct_no_action_skill(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [skill_id] | [skill_id, _] if *skill_id > 0)
}

pub(super) fn supports_direct_skill_card(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [skill_id, 0] if *skill_id > 0)
}

pub(super) fn supports_group_and_star_skill(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [group, star, ..] if match group {
        1 | 2 => (1..=3).contains(star),
        3 => matches!(star, 0 | 1 | 4),
        _ => false,
    })
}

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        match behavior.spec.kind {
            BehaviorKind::ConsumeBuffUseSkill => {
                let [buff_id, amount, skill_id, extra_kind] = behavior.args.as_slice() else {
                    return None;
                };
                if context
                    .managers
                    .buff
                    .buff_id_amount(context.source_uid, *buff_id)
                    < *amount
                {
                    return Some(Vec::new());
                }
                let origin = super::command_origin(behavior)?;
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: context.source_uid,
                        skill_id: *skill_id,
                    }
                    .into();
                invocation.target =
                    crate::engine::skill::action::SkillTarget::Explicit(context.target_uid);
                invocation.extra_skill_kind = skill_kind_from_is_extra(*extra_kind);
                if invocation
                    .extra_skill_kind
                    .is_some_and(|kind| kind.is_extra_action())
                {
                    invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
                }
                Some(vec![
                    RuleOp::Skill(invocation),
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                        origin,
                        target_uid: context.source_uid,
                        selector: BuffSelector::ExactId(*buff_id),
                        amount: *amount,
                        depleted: DepletedBuff::Remove,
                    }))),
                ])
            }
            BehaviorKind::ConsumeBuffUseSkill3 => {
                let [buff_id, amount, skill_id] = behavior.args.as_slice() else {
                    return None;
                };
                if context
                    .managers
                    .buff
                    .buff_id_amount(context.source_uid, *buff_id)
                    < *amount
                {
                    return Some(Vec::new());
                }
                let origin = super::command_origin(behavior)?;
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: context.source_uid,
                        skill_id: *skill_id,
                    }
                    .into();
                invocation.target =
                    crate::engine::skill::action::SkillTarget::Explicit(context.target_uid);
                Some(vec![
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                        origin,
                        target_uid: context.source_uid,
                        selector: BuffSelector::ExactId(*buff_id),
                        amount: *amount,
                        depleted: DepletedBuff::Remove,
                    }))),
                    RuleOp::Skill(invocation),
                ])
            }
            BehaviorKind::ConsumeTargetBuffUseSkill => {
                let [buff_id, amount, skill_id] = behavior.args.as_slice() else {
                    return None;
                };
                if context
                    .managers
                    .buff
                    .buff_id_amount(context.target_uid, *buff_id)
                    < *amount
                {
                    return Some(Vec::new());
                }
                let origin = super::command_origin(behavior)?;
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: context.source_uid,
                        skill_id: *skill_id,
                    }
                    .into();
                invocation.target =
                    crate::engine::skill::action::SkillTarget::Explicit(context.target_uid);
                invocation.extra_skill_kind = Some(ExtraSkillKind::FollowUp);
                invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
                Some(vec![
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                        origin,
                        target_uid: context.target_uid,
                        selector: BuffSelector::ExactId(*buff_id),
                        amount: *amount,
                        depleted: DepletedBuff::Remove,
                    }))),
                    RuleOp::Skill(invocation),
                ])
            }
            BehaviorKind::ConsumePowerDirectUseSkill => {
                let [cost, skill_id] = behavior.args.as_slice() else {
                    return Some(Vec::new());
                };
                if *cost <= 0
                    || *skill_id <= 0
                    || context
                        .managers
                        .eureka
                        .get(context.source_uid, EUREKA_RESOURCE_ID)
                        .current
                        < *cost
                {
                    return Some(Vec::new());
                }
                let origin = super::command_origin(behavior)?;
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: context.source_uid,
                        skill_id: *skill_id,
                    }
                    .into();
                invocation.target =
                    crate::engine::skill::action::SkillTarget::Explicit(context.target_uid);
                Some(vec![
                    RuleOp::Command(BattleCommand::Eureka(EurekaCommand::Change(EurekaChange {
                        origin,
                        source_uid: context.source_uid,
                        target_uid: context.source_uid,
                        power_id: EUREKA_RESOURCE_ID,
                        delta: -*cost,
                        effect_type: sonettobuf::effect_type_enum::EffectType::Powerchange as i32,
                    }))),
                    RuleOp::Skill(invocation),
                ])
            }
            BehaviorKind::DirectUseSkill => {
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: context.source_uid,
                        skill_id: behavior.arg(0)?,
                    }
                    .into();
                invocation.target =
                    crate::engine::skill::action::SkillTarget::Explicit(context.target_uid);
                invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
                Some(vec![RuleOp::Skill(invocation)])
            }
            BehaviorKind::DirectUseSkill2 => {
                let [skill_id, _, _, extra_kind] = behavior.args.as_slice() else {
                    return None;
                };
                if *skill_id <= 0 {
                    return None;
                }
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: context.source_uid,
                        skill_id: *skill_id,
                    }
                    .into();
                invocation.target =
                    crate::engine::skill::action::SkillTarget::Explicit(context.target_uid);
                invocation.extra_skill_kind = skill_kind_from_is_extra(*extra_kind);
                Some(vec![RuleOp::Skill(invocation)])
            }
            BehaviorKind::DirectUseSkillPrev => {
                if context.target.active_skill_source_uid == 0
                    || context.target.active_skill_id == 0
                {
                    return Some(Vec::new());
                }
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: context.target.active_skill_source_uid,
                        skill_id: context.target.active_skill_id,
                    }
                    .into();
                invocation.target =
                    crate::engine::skill::action::SkillTarget::Explicit(context.target_uid);
                invocation.start = crate::engine::skill::action::SkillStart::AfterCurrentAction;
                Some(vec![RuleOp::Skill(invocation)])
            }
            BehaviorKind::DirectUseSkillCard => {
                let [skill_id, 0] = behavior.args.as_slice() else {
                    return None;
                };
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: context.source_uid,
                        skill_id: *skill_id,
                    }
                    .into();
                invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
                Some(vec![RuleOp::Skill(invocation)])
            }
            BehaviorKind::DirectUseSkillNoAct => direct_no_action_skill(context, behavior),
            BehaviorKind::DirectUseSkillNoAct2 => direct_no_action_skill(context, behavior),
            BehaviorKind::DirectUseBigSkill => direct_big_skill_rule_ops(context, behavior),
            BehaviorKind::DirectUseGroupAndStarSkill => {
                let [group, star, ..] = behavior.args.as_slice() else {
                    return Some(Vec::new());
                };
                let Some(skill_id) =
                    skill_from_group_and_star(context.pool, context.source_uid, *group, *star)
                else {
                    return Some(Vec::new());
                };
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: context.source_uid,
                        skill_id,
                    }
                    .into();
                invocation.target =
                    crate::engine::skill::action::SkillTarget::Explicit(context.target_uid);
                invocation.extra_skill_kind =
                    skill_kind_from_is_extra(nested_skill_kind(&behavior.args));
                Some(vec![RuleOp::Skill(invocation)])
            }
            BehaviorKind::RandomUseSkill => {
                let Some(skill_id) = choose_weighted_skill(context.determinism, behavior) else {
                    return Some(Vec::new());
                };
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: context.source_uid,
                        skill_id,
                    }
                    .into();
                invocation.target =
                    crate::engine::skill::action::SkillTarget::Explicit(context.target_uid);
                invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
                Some(vec![RuleOp::Skill(invocation)])
            }
            BehaviorKind::CrystalReuse => {
                let [chance, skill_id, crystal_type] = behavior.args.as_slice() else {
                    return Some(Vec::new());
                };
                let count = usize::try_from(*crystal_type - 1)
                    .ok()
                    .and_then(|index| {
                        context
                            .managers
                            .emanation
                            .counts(context.source_uid)
                            .get(index)
                            .copied()
                    })
                    .unwrap_or_default()
                    .max(0);
                let configured_chance = chance.saturating_mul(count);
                if *skill_id <= 0 || configured_chance <= 0 {
                    return Some(Vec::new());
                }
                let candidates = [*skill_id];
                let captured = context.determinism.take_random_skill(&candidates).is_some();
                let scripted = context.determinism.has_scripted_random_skill(&candidates);
                if !captured && (scripted || !context.determinism.roll_permille(configured_chance))
                {
                    return Some(Vec::new());
                }
                let mut invocation: crate::engine::skill::action::SkillInvocation =
                    crate::engine::skill::action::SkillRequest {
                        source_uid: context.source_uid,
                        skill_id: *skill_id,
                    }
                    .into();
                invocation.extra_skill_kind = skill_kind_from_is_extra(
                    crate::engine::skill::effect::catalog::configured_extra_kind(*skill_id),
                );
                invocation.start = crate::engine::skill::action::SkillStart::AfterCurrentAction;
                if invocation
                    .extra_skill_kind
                    .is_some_and(|kind| kind.is_extra_action())
                {
                    invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
                }
                Some(vec![RuleOp::Skill(invocation)])
            }
            BehaviorKind::ConsumePowerUseSkill => {
                let [cost, skill_id] = behavior.args.as_slice() else {
                    return Some(Vec::new());
                };
                if *cost <= 0 || *skill_id <= 0 {
                    return Some(Vec::new());
                }
                let iterations = context
                    .managers
                    .eureka
                    .get(context.source_uid, EUREKA_RESOURCE_ID)
                    .current
                    / *cost;
                if iterations <= 0 {
                    return Some(Vec::new());
                }
                let origin = super::command_origin(behavior)?;
                let mut ops = Vec::with_capacity(iterations as usize * 2);
                for _ in 0..iterations {
                    ops.push(RuleOp::Command(BattleCommand::Eureka(
                        EurekaCommand::Change(EurekaChange {
                            origin,
                            source_uid: context.source_uid,
                            target_uid: context.source_uid,
                            power_id: EUREKA_RESOURCE_ID,
                            delta: -*cost,
                            effect_type: sonettobuf::effect_type_enum::EffectType::Powerchange
                                as i32,
                        }),
                    )));
                    let mut invocation: crate::engine::skill::action::SkillInvocation =
                        crate::engine::skill::action::SkillRequest {
                            source_uid: context.source_uid,
                            skill_id: *skill_id,
                        }
                        .into();
                    invocation.extra_skill_kind = Some(ExtraSkillKind::ExtraAction);
                    invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
                    ops.push(RuleOp::Skill(invocation));
                }
                Some(ops)
            }
            _ => None,
        }
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior)
    }
}

fn direct_big_skill_rule_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    if behavior.spec.kind != BehaviorKind::DirectUseBigSkill {
        return None;
    }
    let skill_id = context
        .pool
        .entity(context.target_uid)
        .and_then(|entity| (entity.ex_skill > 0).then_some(entity.ex_skill))?;
    let consumed = context.managers.ex_point.get(context.target_uid).max(0);
    let refund = consumed
        .min(crate::engine::skill::effect::catalog::configured_big_skill_point(skill_id).max(0));
    let origin = super::command_origin(behavior)?;
    let ex_point = |delta| {
        RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
            ExPointChange {
                origin,
                source_uid: context.target_uid,
                target_uid: context.target_uid,
                delta,
                config_effect: 0,
                effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
            },
        )))
    };
    let mut ops = Vec::with_capacity(3);
    if consumed > 0 {
        ops.push(ex_point(-consumed));
    }
    let mut invocation: crate::engine::skill::action::SkillInvocation =
        crate::engine::skill::action::SkillRequest {
            source_uid: context.target_uid,
            skill_id,
        }
        .into();
    invocation.mode = crate::engine::skill::action::SkillExecutionMode::DirectBig;
    invocation.additional_moxie = consumed;
    ops.push(RuleOp::Skill(invocation));
    if refund > 0 {
        ops.push(ex_point(refund));
    }
    Some(ops)
}

fn references(behavior: &ParsedBehavior) -> RuleReferences {
    let skills = match behavior.spec.kind {
        BehaviorKind::ConsumeBuffUseSkill
        | BehaviorKind::ConsumeBuffUseSkill3
        | BehaviorKind::ConsumeTargetBuffUseSkill => behavior.arg(2).into_iter().collect(),
        BehaviorKind::ConsumePowerUseSkill | BehaviorKind::ConsumePowerDirectUseSkill => {
            behavior.arg(1).into_iter().collect()
        }
        BehaviorKind::DirectUseSkill
        | BehaviorKind::DirectUseSkill2
        | BehaviorKind::DirectUseSkillCard
        | BehaviorKind::DirectUseSkillNoAct
        | BehaviorKind::DirectUseSkillNoAct2
        | BehaviorKind::DirectUseSkillNotExtra
        | BehaviorKind::UseExtraSkill => behavior.arg(0).into_iter().collect(),
        BehaviorKind::RandomUseSkill => behavior
            .raw_args
            .first()
            .and_then(|raw| weighted_skills(raw))
            .unwrap_or_default()
            .into_iter()
            .map(|(skill_id, _)| skill_id)
            .collect(),
        BehaviorKind::CrystalReuse => behavior.arg(1).into_iter().collect(),
        _ => Vec::new(),
    };
    let buffs = match behavior.spec.kind {
        BehaviorKind::ConsumeBuffUseSkill
        | BehaviorKind::ConsumeBuffUseSkill3
        | BehaviorKind::ConsumeTargetBuffUseSkill => behavior.arg(0).into_iter().collect(),
        _ => Vec::new(),
    };
    RuleReferences {
        skills,
        buffs,
        models: Vec::new(),
    }
}

fn direct_no_action_skill(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let mut invocation: crate::engine::skill::action::SkillInvocation =
        crate::engine::skill::action::SkillRequest {
            source_uid: context.source_uid,
            skill_id: behavior.arg(0)?,
        }
        .into();
    invocation.target = crate::engine::skill::action::SkillTarget::Explicit(
        if context.target.runtime_target_uid != 0 {
            context.target.runtime_target_uid
        } else {
            context.target_uid
        },
    );
    Some(vec![RuleOp::Skill(invocation)])
}

pub fn is_consume_power_use_skill(behavior: &ParsedBehavior) -> bool {
    behavior.spec.kind == BehaviorKind::ConsumePowerUseSkill
}

pub fn direct_skill_id_for_power_event(behavior: &ParsedBehavior) -> Option<i32> {
    matches!(
        behavior.spec.kind,
        BehaviorKind::DirectUseSkill
            | BehaviorKind::DirectUseSkillNoAct
            | BehaviorKind::DirectUseSkillNotExtra
            | BehaviorKind::UseExtraSkill
    )
    .then(|| behavior.arg(0))
    .flatten()
}

pub fn resolve_targets(
    skill_id: i32,
    source_uid: i64,
    target_code: i32,
    pool: &TargetPool,
    determinism: &mut RoundDeterminism,
    behavior: &ParsedBehavior,
) -> Option<Vec<i64>> {
    if behavior.spec.kind != BehaviorKind::DirectUseSkill || target_code != 201 {
        return None;
    }
    let candidates = pool
        .main_allies(source_uid)
        .iter()
        .filter(|entity| entity.uid != source_uid)
        .map(|entity| entity.uid)
        .collect::<Vec<_>>();
    if let Some(targets) = determinism.take_skill_targets(skill_id, source_uid, target_code)
        && !targets.is_empty()
        && targets.iter().all(|target| candidates.contains(target))
    {
        return Some(targets);
    }
    Some(
        determinism
            .lua_random_index(candidates.len())
            .map(|index| vec![candidates[index]])
            .unwrap_or_default(),
    )
}

fn weighted_skills(raw: &str) -> Option<Vec<(i32, usize)>> {
    raw.split('&')
        .map(|choice| {
            let (skill, weight) = choice.split_once(':')?;
            let skill = skill.parse().ok()?;
            let weight = weight.parse().ok()?;
            (skill > 0 && weight > 0).then_some((skill, weight))
        })
        .collect()
}

fn choose_weighted_skill(
    determinism: &mut RoundDeterminism,
    behavior: &ParsedBehavior,
) -> Option<i32> {
    let choices = behavior
        .raw_args
        .first()
        .and_then(|raw| weighted_skills(raw))?;
    let candidates = choices
        .iter()
        .map(|(skill_id, _)| *skill_id)
        .collect::<Vec<_>>();
    if let Some(skill_id) = determinism.take_random_skill(&candidates) {
        return Some(skill_id);
    }
    if determinism.has_scripted_random_skill(&candidates) {
        return None;
    }
    let mut roll =
        determinism.lua_random_index(choices.iter().map(|(_, weight)| *weight).sum::<usize>())?;
    for (skill_id, weight) in choices {
        if roll < weight {
            return Some(skill_id);
        }
        roll -= weight;
    }
    None
}

fn nested_skill_kind(args: &[i32]) -> i32 {
    args.get(3)
        .copied()
        .unwrap_or_else(|| ExtraSkillKind::ExtraAction.id())
}

fn skill_from_group_and_star(
    pool: &TargetPool,
    source_uid: i64,
    group: i32,
    star: i32,
) -> Option<i32> {
    let source = pool.entity(source_uid)?;
    let skills = match group {
        1 => Skill::get_skill_groups_with_destiny(source.model_id, 0, None).0,
        2 => Skill::get_skill_groups_with_destiny(source.model_id, 0, None).1,
        3 => return (source.ex_skill > 0).then_some(source.ex_skill),
        _ => return None,
    };
    skills.get(star.saturating_sub(1) as usize).copied()
}

#[cfg(test)]
mod test;
