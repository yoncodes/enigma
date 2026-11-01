use crate::engine::{
    event::payload::BattleEvent,
    manager::{BattleManagers, buff::{BuffCommand, BuffGrant}},
    runtime::determinism::RoundDeterminism,
    skill::{rule::output::{BattleCommand, RuleOp}, subscriber::BuffActSubscriber},
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [chance, buff_id, types @ ..] if (0..=1000).contains(chance)
        && *buff_id > 0 && !types.is_empty() && types.iter().all(|id| matches!(*id, 2 | 4 | 6)))
}

pub fn rule_ops(managers: &BattleManagers, determinism: &mut RoundDeterminism,
    subscriber: &BuffActSubscriber, event: &BattleEvent) -> Option<Vec<RuleOp>> {
    let BattleEvent::AllyAction(action) = event else { return None; };
    if !supports(&subscriber.args) { return None; }
    if subscriber.owner_uid != action.source_uid { return Some(Vec::new()); }
    let [chance, buff_id, types @ ..] = subscriber.args.as_slice() else { return None; };
    let source_uid = if subscriber.source_uid != 0 { subscriber.source_uid } else { subscriber.owner_uid };
    let origin = super::command_origin(subscriber)?;
    let mut ops = Vec::new();
    for &(target_uid, applied_buff_id, count) in &action.buff_addition_targets {
        if managers.entity.team_type(target_uid) == Some(subscriber.team_type) { continue; }
        let Some(status) = managers.catalog().buff_status(applied_buff_id) else { continue; };
        if !types.contains(&(status as i32)) { continue; }
        for _ in 0..count.max(0) {
            if determinism.roll_permille(*chance) {
                ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                    origin, source_uid, target_uid, buff_id: *buff_id, amount: Some(1),
                    occurrences: 1, child_uid_reservations: 0,
                }))));
            }
        }
    }
    Some(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    #[test]
    fn tuesday_rolls_for_each_enemy_status_and_attributes_poison_to_her() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam { entitys: vec![FightEntityInfo {
                uid: Some(10), team_type: Some(1), current_hp: Some(100),
                buffs: vec![BuffInfo { uid: Some(20), buff_id: Some(30980142),
                    from_uid: Some(11), ..Default::default() }], ..Default::default()
            }], ..Default::default() }),
            defender: Some(FightTeam { entitys: vec![FightEntityInfo {
                uid: Some(-1), team_type: Some(2), current_hp: Some(100), ..Default::default()
            }], ..Default::default() }), ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let feature = managers.buff.active_features_for_owner(&managers.hp, 10).into_iter()
            .find(|f| super::super::is_kind(f, super::super::registry::BuffActKind::ProbabilityAddBuff)).unwrap();
        let subscriber = super::super::subscriber_from_feature(feature, crate::engine::event::kind::EventKind::AllyAction).unwrap();
        let event = BattleEvent::AllyAction(crate::engine::skill::action::ActionEvent {
            source_uid: 10, buff_addition_targets: vec![(-1, 31420000, 2)], ..Default::default()
        });
        let mut random = RoundDeterminism::default();
        let other = BattleEvent::AllyAction(crate::engine::skill::action::ActionEvent {
            source_uid: 11, buff_addition_targets: vec![(-1, 31420000, 1)], ..Default::default()
        });
        assert!(rule_ops(&managers, &mut random, &subscriber, &other).unwrap().is_empty());
        random.enqueue_permille_rolls([100, 900]);
        let ops = rule_ops(&managers, &mut random, &subscriber, &event).unwrap();
        assert!(matches!(ops.as_slice(), [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))]
            if grant.source_uid == 11 && grant.target_uid == -1 && grant.buff_id == 30980145));
    }
}
