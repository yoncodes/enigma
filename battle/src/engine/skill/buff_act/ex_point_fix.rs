use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        ex_point::{ExPointChange, ExPointCommand},
    },
    skill::{rule::output::{BattleCommand, RuleOp}, subscriber::BuffActSubscriber},
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [target, amount] if matches!(*target, 0 | 1) && *amount > 0)
}

pub fn rule_ops(managers: &BattleManagers, subscriber: &BuffActSubscriber, event: &BattleEvent)
    -> Option<Vec<RuleOp>>
{
    if !super::subscriber_is_kind(subscriber, super::registry::BuffActKind::ExPointFix)
        || !supports(&subscriber.args)
    {
        return None;
    }
    let BattleEvent::Hit(hit) = event else { return Some(Vec::new()) };
    if hit.target_uid != subscriber.owner_uid {
        return Some(Vec::new());
    }
    let target_uid = if subscriber.args[0] == 1 { hit.source_uid } else { subscriber.owner_uid };
    let origin = super::command_origin(subscriber)?;
    let mut ops = vec![RuleOp::Command(BattleCommand::ExPoint(ExPointCommand::Change(
        ExPointChange { origin, source_uid: subscriber.owner_uid, target_uid,
            delta: subscriber.args[1], config_effect: 0,
            effect_type: sonettobuf::effect_type_enum::EffectType::Expointchange as i32 },
    )))];
    if managers.catalog().buff_has_effect_count(subscriber.buff_id) {
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeCount(
            BuffConsume { origin, target_uid: subscriber.owner_uid,
                selector: BuffSelector::Uid(subscriber.buff_uid), amount: 1,
                depleted: DepletedBuff::Remove },
        ))));
    }
    Some(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};
    use crate::engine::{
        event::payload::HitEvent,
        manager::hp::HurtDamageFromType,
        runtime::{determinism::RoundDeterminism, drain::run_event},
        skill::{effect::SkillEffectCatalog, rule::{CommandOrigin, DefinitionKey, RuleDomain},
            target::{TargetContext, TargetPool}},
    };

    #[test]
    fn noise_grants_attacker_moxie_once_and_consumes_the_buff() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam { entitys: vec![FightEntityInfo {
                uid: Some(10), team_type: Some(1), current_hp: Some(1000), ex_point: Some(0),
                attr: Some(HeroAttribute { hp: Some(1000), ..Default::default() }),
                ..Default::default()
            }], ..Default::default() }),
            defender: Some(FightTeam { entitys: vec![FightEntityInfo {
                uid: Some(-2), team_type: Some(2), current_hp: Some(1000),
                attr: Some(HeroAttribute { hp: Some(1000), ..Default::default() }),
                buffs: vec![BuffInfo { uid: Some(20), buff_id: Some(123100121),
                    from_uid: Some(-1), layer: Some(1), count: Some(1), ..Default::default() }],
                ..Default::default()
            }], ..Default::default() }), ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
        let hit = BattleEvent::Hit(HitEvent {
            origin: CommandOrigin { domain: RuleDomain::Skill, key: DefinitionKey::new(1, "SkillDamage") },
            source_uid: 10, target_uid: -2, skill_id: 30410111, amount: 50,
            resolved_damage: 50, shield_absorbed: 0, career_restraint: false,
            damage_from: HurtDamageFromType::Skill, share_count: 0, assassinate: false,
            ignore_riposte: false,
        });
        for _ in 0..2 {
            run_event(&mut managers, &pool, &catalog, &mut RoundDeterminism::default(),
                TargetContext::default(), hit.clone()).unwrap();
        }
        assert_eq!(managers.ex_point.get(10), 1);
        assert!(managers.buff.buff_id_uid(-2, 123100121).is_none());
    }
}
