use crate::engine::{
    mechanic::focus_all_entity_buff::FocusAllEntityBuffCommand,
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

pub fn supports_arguments(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [buff_id] if *buff_id > 0)
}

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let buff_id = behavior.arg(0)?;
        if buff_id <= 0 {
            return None;
        }
        let capacity = context.managers.buff.stack_limit(buff_id);
        let current = context
            .managers
            .buff
            .buff_id_amount(context.target_uid, buff_id);
        if current <= 0 || current >= capacity {
            return Some(Vec::new());
        }
        let candidate_uids = context
            .pool
            .active_entities()
            .map(|entity| entity.uid)
            .filter(|uid| {
                *uid != context.source_uid
                    && context.managers.buff.has_active_buff_id(*uid, buff_id)
            })
            .take((capacity - current) as usize)
            .collect::<Vec<_>>();
        if candidate_uids.is_empty() {
            return Some(Vec::new());
        }
        Some(vec![RuleOp::Command(BattleCommand::FocusAllEntityBuff(
            FocusAllEntityBuffCommand {
                origin: super::command_origin(behavior)?,
                source_uid: context.source_uid,
                target_uid: context.target_uid,
                buff_id,
                candidate_uids,
            },
        ))])
    }
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        manager::BattleManagers,
        runtime::determinism::RoundDeterminism,
        skill::{
            action::SkillModifiers,
            behavior::{self, classify::BehaviorSpec},
            target::{TargetContext, TargetPool},
        },
    };

    fn entity(uid: i64, team_type: i32, buff_uid: i64) -> FightEntityInfo {
        FightEntityInfo {
            uid: Some(uid),
            team_type: Some(team_type),
            current_hp: Some(100),
            buffs: vec![BuffInfo {
                buff_id: Some(303901411),
                uid: Some(buff_uid),
                layer: Some(1),
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn emits_ordered_cross_team_candidates_and_excludes_source() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    entity(-1, 1, 1039),
                    entity(-2, 1, 1040),
                    entity(-3, 1, 1041),
                ],
                sub_entitys: vec![entity(-8, 1, 1046)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-5, 2, 1043)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(60263, "FocusAllEntityBuff"),
            vec![303901411],
            Vec::new(),
        );

        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: -3,
                source_team: 1,
                target_uid: -3,
                active_skill_id: 303901431,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::FocusAllEntityBuff(
                FocusAllEntityBuffCommand { candidate_uids, .. }
            ))] if candidate_uids == &[-1, -2, -5]
        ));
    }
}
