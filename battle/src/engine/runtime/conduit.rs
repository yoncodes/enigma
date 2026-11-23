use crate::engine::{
    manager::{BattleManagers, conduit::ConduitSkill},
    runtime::{determinism::RoundDeterminism, drain},
    skill::{
        action::{SkillInvocation, SkillRequest},
        effect::SkillEffectCatalog,
        rule::output::RuleOp,
        target::{TargetContext, TargetPool},
    },
};

pub(crate) struct Admission {
    pub cost: i32,
    pub cost_modifier: Option<(i32, RuleOp)>,
    pub funded: bool,
    pub runtime_ready: bool,
    pub selectable: bool,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn admit(
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    context: TargetContext,
    source_uid: i64,
    skill: ConduitSkill,
    free_skill_used: bool,
) -> Option<Admission> {
    let cost_modifier = crate::engine::skill::buff_act::device_cost_reduce::modifier(
        managers,
        source_uid,
        skill.skill_id,
    );
    let reduction = cost_modifier
        .as_ref()
        .map(|(reduction, _)| *reduction)
        .unwrap_or_default();
    let invocation: SkillInvocation = SkillRequest {
        source_uid,
        skill_id: skill.skill_id,
    }
    .into();
    let runtime_ready = !catalog.is_big_skill(skill.skill_id)
        || crate::engine::manager::ex_point::ExPointKind::from_wire(
            managers.ex_point.kind(source_uid),
        ) != crate::engine::manager::ex_point::ExPointKind::DevicePower
        || managers.ex_point.is_full(source_uid);
    let unavailable = skill.is_stopped
        || (skill.cost_value <= 0 && free_skill_used)
        || drain::attack_has_no_target(&invocation, catalog, pool, managers, determinism, context);
    (!unavailable).then_some(Admission {
        cost: skill.cost_after_reduction(reduction),
        cost_modifier,
        funded: managers.conduit.can_pay(source_uid, skill, reduction),
        runtime_ready,
        selectable: !catalog.is_big_skill(skill.skill_id)
            || pool.entity(source_uid).is_some_and(|source| {
                crate::engine::mechanic::card::CardMechanic
                    .ultimate_resource_ready(managers, source)
            }),
    })
}
