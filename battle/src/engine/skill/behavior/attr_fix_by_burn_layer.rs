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
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        manager::{BattleManagers, hp::HpCommand},
        runtime::determinism::RoundDeterminism,
        skill::{
            action::{SkillActionEvent, SkillPhase},
            behavior::BehaviorOpContext,
            target::{TargetContext, TargetPool},
        },
    };

    fn fight() -> Fight {
        Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(10_000),
                    attr: Some(HeroAttribute {
                        hp: Some(10_000),
                        attack: Some(1_644),
                        ..Default::default()
                    }),
                    buffs: vec![
                        BuffInfo {
                            uid: Some(20),
                            buff_id: Some(4150001),
                            count: Some(1),
                            layer: Some(6),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(21),
                            buff_id: Some(30940151),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn current_attack_scales_with_burn_and_resolves_it_without_consuming_stacks() {
        crate::test_support::init_config();
        let fight = fight();
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let behavior = ParsedBehavior::new(
            60065,
            "AttrFixByBurnLayerAndExtraBurnHurt",
            vec![1, AttrId::Attack.id(), 5, -1],
        );
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
        let mut target = TargetContext::default();

        assert!(Handler::collect_attack_modifier(
            AttackModifierContext {
                operation: BehaviorOpContext {
                    source_uid: 10,
                    source_team: 1,
                    target_uid: 0,
                    active_skill_id: 30940111,
                    transfer_count: 1,
                    event: None,
                    managers: &managers,
                    pool: &pool,
                    determinism: &mut determinism,
                    modifiers: &mut modifiers,
                    target: &mut target,
                },
                owner_skill_id: 0,
                conditions: &[],
            },
            &behavior,
        ));
        assert!(modifiers.attack_attributes.is_empty());
        assert_eq!(modifiers.caster_attack_bonus, 49);

        let event = BattleEvent::SkillAction(SkillActionEvent {
            source_uid: 10,
            card_index: 0,
            skill_id: 30940111,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase: SkillPhase::Immediate,
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            skill_type: 1,
            effect_tag: 0,
            assassinate: false,
            ignore_riposte: false,
            damage_amount: 0,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode: crate::engine::skill::action::SkillExecutionMode::Active,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        });
        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 30940111,
                transfer_count: 1,
                event: Some(&event),
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(ops.iter().any(|op| matches!(
            op,
            RuleOp::Command(crate::engine::skill::rule::output::BattleCommand::Hp(
                HpCommand::Lose(loss)
            )) if loss.target_uid == 10 && loss.amount == 197
        )));
        assert_eq!(managers.buff.buff_id_amount(10, 4150001), 6);
    }
}
