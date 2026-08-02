use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::Hit(hit) = event else {
        return None;
    };
    if hit.target_uid != subscriber.owner_uid
        || hit.amount <= 0
        || hit.damage_from == HurtDamageFromType::ShareHurt
    {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    let shared = pool
        .main_allies(subscriber.owner_uid)
        .iter()
        .filter(|ally| ally.uid != subscriber.owner_uid && managers.hp.current(ally.uid) > 0)
        .map(|ally| {
            HpCommand::Lose(HpLoss {
                origin,
                source_uid: hit.source_uid,
                target_uid: ally.uid,
                amount: hit.amount,
                config_effect: -1,
                hurt: Some(HurtInfoData {
                    from_uid: hit.source_uid,
                    is_crit: false,
                    career_restraint: false,
                    reduce_hp: 0,
                    effect_id: 0,
                    skill_id: 0,
                    damage_from: HurtDamageFromType::ShareHurt,
                    buff_act_id: 0,
                    buff_uid: 0,
                    hurt_effect_type: EffectType::Sharehurt as i32,
                    display_amount: None,
                }),
            })
        })
        .collect::<Vec<_>>();
    if shared.is_empty() {
        return Some(Vec::new());
    }
    Some(vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::Uid(subscriber.buff_uid),
            amount: 1,
            depleted: DepletedBuff::Remove,
        }))),
        RuleOp::Command(BattleCommand::HpBatch(shared)),
    ])
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        event::payload::HitEvent,
        manager::BattleManagers,
        skill::{
            rule::{CommandOrigin, DefinitionKey, RuleDomain},
            subscriber::BuffActSubscriber,
        },
    };

    #[test]
    fn shares_the_committed_hit_with_other_main_allies_and_consumes_one_stack() {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: [10, 11, 12]
                    .into_iter()
                    .map(|uid| FightEntityInfo {
                        uid: Some(uid),
                        current_hp: Some(1_000),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let managers = BattleManagers::seeded(&fight);
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 50,
            buff_id: 31090121,
            team_type: 1,
            owner_alive: true,
            amount: 2,
            key: crate::engine::event::subscription::SubscriptionKey::new(
                crate::engine::event::kind::EventKind::BeAttacked,
                DefinitionKey::new(872, "ShareHurt"),
            ),
            act_type: "ShareHurt".to_owned(),
            effect_time: 209,
            effect_condition: 0,
            args: Vec::new(),
            raw: "872".to_owned(),
        };
        let event = BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "SkillDamage"),
            },
            source_uid: -1,
            target_uid: 10,
            skill_id: 1,
            amount: 83,
            shield_absorbed: 0,
            damage_from: HurtDamageFromType::Skill,
            assassinate: false,
            ignore_riposte: false,
        });

        let ops = rule_ops(&managers, &pool, &subscriber, &event).unwrap();

        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                    selector: BuffSelector::Uid(50),
                    amount: 1,
                    ..
                }))),
                RuleOp::Command(BattleCommand::HpBatch(shared))
            ] if matches!(
                shared.as_slice(),
                [
                    HpCommand::Lose(HpLoss { target_uid: 11, amount: 83, .. }),
                    HpCommand::Lose(HpLoss { target_uid: 12, amount: 83, .. })
                ]
            )
        ));
    }
}
