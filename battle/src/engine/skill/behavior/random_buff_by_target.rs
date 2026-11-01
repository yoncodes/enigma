use crate::engine::{
    manager::buff::{BuffCommand, BuffGrant, BuffStatus},
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::{RuleReferences, output::{BattleCommand, RuleOp}},
    },
};

pub(super) struct Handler;

pub(super) fn supports(behavior: &ParsedBehavior) -> bool {
    matches!(behavior.args.as_slice(), [6, buff_id, 1] if *buff_id > 0)
}

impl BehaviorHandler for Handler {
    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        RuleReferences { buffs: behavior.arg(1).into_iter().collect(), ..Default::default() }
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let [6, buff_id, 1] = behavior.args.as_slice() else { return None };
        let allies = context.pool.active_entities()
            .filter(|entity| context.pool.team_type(entity.uid) == Some(context.source_team))
            .map(|entity| entity.uid).collect::<Vec<_>>();
        if allies.is_empty() { return Some(Vec::new()); }
        let affected = context.pool.active_entities()
            .filter(|entity| context.pool.team_type(entity.uid).is_some_and(|team| team != context.source_team))
            .filter(|entity| !context.managers.buff.buffs_with_status(entity.uid, BuffStatus::NegativeStatus).is_empty())
            .count();
        let origin = super::command_origin(behavior)?;
        let mut ops = Vec::with_capacity(affected);
        for _ in 0..affected {
            let target_uid = allies[context.determinism.lua_random_index(allies.len())?];
            ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                origin, source_uid: context.source_uid, target_uid, buff_id: *buff_id,
                amount: None, occurrences: 1, child_uid_reservations: 0,
            }))));
        }
        Some(ops)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};
    use crate::engine::{manager::BattleManagers, runtime::determinism::RoundDeterminism,
        skill::{action::SkillModifiers, behavior::classify::BehaviorSpec,
            target::{TargetContext, TargetPool}}};

    #[test]
    fn each_enemy_with_negative_status_grants_one_buff_to_a_living_ally() {
        crate::test_support::init_config();
        let entity = |uid, team| FightEntityInfo { uid: Some(uid), team_type: Some(team),
            current_hp: Some(100), ..Default::default() };
        let debuff = |uid| BuffInfo { uid: Some(uid), buff_id: Some(31320113),
            from_uid: Some(10), ..Default::default() };
        let fight = Fight {
            attacker: Some(FightTeam { entitys: vec![entity(10, 1), entity(11, 1)], ..Default::default() }),
            defender: Some(FightTeam { entitys: vec![
                FightEntityInfo { buffs: vec![debuff(100)], ..entity(-1, 2) },
                FightEntityInfo { buffs: vec![debuff(101)], ..entity(-2, 2) },
                entity(-3, 2),
            ], ..Default::default() }), ..Default::default() };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let mut random = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(BehaviorSpec::new(60166, "RandomAddBuffByTargetBuffType"),
            vec![6, 5022, 1], vec!["6".into(), "5022".into(), "1".into()]);
        let ops = Handler::emit_ops(BehaviorOpContext { source_uid: 10, source_team: 1,
            target_uid: 10, active_skill_id: 30390150, transfer_count: 1, event: None,
            managers: &managers, pool: &pool, determinism: &mut random,
            modifiers: &mut modifiers, target: &mut target }, &behavior).unwrap();
        assert_eq!(ops.len(), 2);
        assert!(ops.iter().all(|op| matches!(op,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
                if [10, 11].contains(&grant.target_uid) && grant.buff_id == 5022)));
    }
}
