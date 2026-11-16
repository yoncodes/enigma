use crate::engine::{
    manager::ex_point::{ExPointChange, ExPointKind},
    skill::{
        action::{PaidUltimate, SkillExecutionMode, SkillInvocation, SkillRequest},
        behavior::BehaviorOpContext,
        effect::{ParsedBehavior, SkillEffectSlot},
        rule::{CommandOrigin, output::RuleOp},
        target::request::SOURCE_TARGET_CODE,
    },
};

pub(super) struct Args {
    cost: i32,
    skill: i32,
    marker: i32,
}

pub(super) fn parse(behavior: &ParsedBehavior) -> Option<Args> {
    if behavior.raw_arity != 4 || behavior.arg(2)? != 1 {
        return None;
    }
    let (cost, skill, marker) = (behavior.arg(0)?, behavior.arg(1)?, behavior.arg(3)?);
    (cost > 0 && skill > 0 && marker > 0).then_some(Args {
        cost,
        skill,
        marker,
    })
}

pub(super) fn supports_slot(slot: &SkillEffectSlot) -> bool {
    slot.condition_target.code == SOURCE_TARGET_CODE
        && slot.condition_target.raw.is_empty()
        && slot.target.code == SOURCE_TARGET_CODE
        && slot.target.raw.is_empty()
        && !slot.target_from_condition
        && matches!(slot.conditions.as_slice(), [condition] if
            condition.opcode == 302 && condition.type_name == "None" && condition.raw_args.is_empty()
        )
}

impl Args {
    pub(super) fn emit(self, context: BehaviorOpContext<'_>, origin: CommandOrigin) -> Vec<RuleOp> {
        let uid = context.source_uid;
        if ExPointKind::from_wire(context.managers.ex_point.kind(uid)) != ExPointKind::Common {
            return Vec::new();
        }
        let count = context.managers.ex_point.get(uid) / self.cost;
        let mut invocation = SkillInvocation::from(SkillRequest {
            source_uid: uid,
            skill_id: self.skill,
        });
        invocation.mode = SkillExecutionMode::Direct;
        invocation.paid_ultimate = Some(PaidUltimate {
            cost: ExPointChange {
                origin,
                source_uid: uid,
                target_uid: uid,
                delta: -self.cost,
                config_effect: 0,
                effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32,
            },
            priority_buff: self.marker,
        });
        (0..count)
            .map(|_| RuleOp::Skill(invocation.clone()))
            .collect()
    }
}
