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

pub(super) struct SkillAdmission {
    pub cost_modifier: Option<(i32, RuleOp)>,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn admit_skill(
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &RoundDeterminism,
    context: TargetContext,
    source_uid: i64,
    skill: ConduitSkill,
    free_skill_used: bool,
) -> Option<SkillAdmission> {
    let cost_modifier = crate::engine::skill::buff_act::device_cost_reduce::modifier(
        managers,
        source_uid,
        skill.skill_id,
    );
    let cost_reduction = cost_modifier
        .as_ref()
        .map(|(reduction, _)| *reduction)
        .unwrap_or_default();
    let invocation: SkillInvocation = SkillRequest {
        source_uid,
        skill_id: skill.skill_id,
    }
    .into();
    let unavailable = (skill.cost_value <= 0 && free_skill_used)
        || (catalog.is_big_skill(skill.skill_id)
            && crate::engine::manager::ex_point::ExPointKind::from_wire(
                managers.ex_point.kind(source_uid),
            ) == crate::engine::manager::ex_point::ExPointKind::DevicePower
            && !managers.ex_point.is_full(source_uid))
        || !managers
            .conduit
            .can_begin_skill(source_uid, skill.skill_id, cost_reduction)
        || drain::attack_has_no_target(&invocation, catalog, pool, managers, determinism, context);
    (!unavailable).then_some(SkillAdmission { cost_modifier })
}
