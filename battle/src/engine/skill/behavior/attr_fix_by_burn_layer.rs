use crate::engine::{
    damage::scale_permille_stacks,
    entity::attr::AttrId,
    event::{kind::EventKind, payload::BattleEvent},
    skill::{
        behavior::{AttackModifierContext, BehaviorOpContext, registry::BehaviorHandler},
        buff_act::{self, registry::BuffActKind},
        effect::ParsedBehavior,
        rule::output::RuleOp,
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        attack_bonus_args(behavior).is_some()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, _: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let Some(BattleEvent::SkillAction(action)) = context.event else {
            return Some(Vec::new());
        };
        if action.source_uid != context.source_uid
            || action.phase != crate::engine::skill::action::SkillPhase::Immediate
        {
            return Some(Vec::new());
        }

        let mut ops = Vec::new();
        for feature in context.managers.buff.active_features(&context.managers.hp) {
            if feature.owner_uid != context.source_uid
                || !buff_act::is_kind(&feature, BuffActKind::Burn)
            {
                continue;
            }
            let subscriber = buff_act::subscriber_from_feature(feature, EventKind::RoundEnd)?;
            ops.extend(buff_act::damage_over_time::damage_rule_ops(
                context.managers,
                context.pool,
                context.determinism,
                &subscriber,
            ));
        }
        Some(ops)
    }

    fn collect_attack_modifier(
        context: AttackModifierContext<'_>,
        behavior: &ParsedBehavior,
    ) -> bool {
        let Some((rate, cap)) = attack_bonus_args(behavior) else {
            return false;
        };
        let managers = context.operation.managers;
        let source_uid = context.operation.source_uid;
        let stacks = burn_stacks(managers, source_uid);
        let stacks = if cap < 0 { stacks } else { stacks.min(cap) };
        if context.operation.active_skill_id != 0 && stacks > 0 {
            let base = managers.attribute.base(source_uid, AttrId::Attack);
            let bonus = scale_permille_stacks(base, rate, stacks);
            let attack_bonus = &mut context.operation.modifiers.caster_attack_bonus;
            *attack_bonus = attack_bonus.saturating_add(bonus);
        }
        true
    }
}

fn attack_bonus_args(behavior: &ParsedBehavior) -> Option<(i32, i32)> {
    let [1, attr, rate, cap] = behavior.args.as_slice() else {
        return None;
    };
    (*attr == AttrId::Attack.id() && *rate > 0 && (*cap == -1 || *cap > 0)).then_some((*rate, *cap))
}

fn burn_stacks(managers: &crate::engine::manager::BattleManagers, owner_uid: i64) -> i32 {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == owner_uid && buff_act::is_kind(feature, BuffActKind::Burn)
        })
        .map(|feature| feature.amount)
        .sum()
}

#[cfg(test)]
mod tests;
