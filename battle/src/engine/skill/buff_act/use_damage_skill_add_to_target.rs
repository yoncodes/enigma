use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffGrant, BuffSelector, DepletedBuff},
    },
    skill::{
        action::SkillPhase,
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn rule_ops(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<super::BuffActRuleOp>> {
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::UseDamageSkillAddToTarget,
    ) {
        return None;
    }
    let (source_uid, target_uids) = match event {
        BattleEvent::SkillAction(action)
            if action.source_uid == subscriber.owner_uid
                && action.phase == SkillPhase::Immediate
                && catalog.damage_rate(action.skill_id) > 0 =>
        {
            (action.source_uid, Some(action.target_uids.as_slice()))
        }
        BattleEvent::SkillAction(action)
            if action.source_uid == subscriber.owner_uid
                && action.phase == SkillPhase::HitPassives
                && subscriber.key.event == crate::engine::event::kind::EventKind::SkillCast
                && catalog.damage_rate(action.skill_id) > 0 =>
        {
            (action.source_uid, None)
        }
        _ => return Some(Vec::new()),
    };
    let [_, buff_id, ..] = subscriber.args.as_slice() else {
        return None;
    };
    let stored = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let activation_count = stored.count.unwrap_or_default();
    if activation_count <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    if target_uids.is_none() {
        return Some(vec![super::BuffActRuleOp::subscriber_from_applier(
            RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeCount(
                BuffConsume {
                    origin,
                    target_uid: subscriber.owner_uid,
                    selector: BuffSelector::Uid(subscriber.buff_uid),
                    amount: activation_count,
                    depleted: DepletedBuff::Remove,
                },
            ))),
        )]);
    }
    let stacks = stored.layer.unwrap_or_default().max(0);
    if stacks == 0 || *buff_id <= 0 {
        return Some(Vec::new());
    }
    let occurrences = u32::try_from(stacks).ok()?;
    let mut ops = Vec::new();
    for target_uid in target_uids
        .expect("skill-action branch has targets")
        .iter()
        .copied()
        .filter(|target_uid| *target_uid != 0)
    {
        ops.push(super::BuffActRuleOp::causing(RuleOp::Command(
            BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                origin,
                source_uid,
                target_uid,
                buff_id: *buff_id,
                amount: None,
                occurrences,
                child_uid_reservations: 0,
            })),
        )));
        ops.push(super::BuffActRuleOp::causing(RuleOp::BuffActTrigger(
            crate::engine::manager::buff::BuffActTriggerResult {
                target_uid: source_uid,
                buff_id: subscriber.buff_id,
                buff_act_id: subscriber.key.definition.opcode,
            },
        )));
    }
    Some(ops)
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::rule::DefinitionKey,
    };

    #[test]
    fn carrier_is_consumed_after_the_hit_batch_at_hit_passives() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    buffs: vec![BuffInfo {
                        uid: Some(63),
                        buff_id: Some(4_150_002),
                        from_uid: Some(20),
                        count: Some(1),
                        layer: Some(6),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(1_000),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 20,
            buff_uid: 63,
            buff_id: 4_150_002,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::SkillCast,
                DefinitionKey::new(748, "UseDamageSkillAddToTarget"),
            ),
            act_type: "UseDamageSkillAddToTarget".to_owned(),
            effect_time: 201,
            effect_condition: 0,
            args: vec![0, 4_150_001],
            raw: "748#0#4150001".to_owned(),
        };
        let mut catalog = SkillEffectCatalog::default();
        catalog.insert_damage_rate(100, 1_000);
        let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
            source_uid: 10,
            skill_id: 100,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase: SkillPhase::HitPassives,
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            skill_type: 0,
            effect_tag: 1,
            assassinate: false,
            ignore_riposte: false,
            damage_amount: 100,
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

        let ops = rule_ops(&managers, &catalog, &subscriber, &event).unwrap();

        assert!(matches!(
            ops.as_slice(),
            [super::super::BuffActRuleOp {
                op: RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeCount(
                    BuffConsume {
                        target_uid: 10,
                        selector: BuffSelector::Uid(63),
                        amount: 1,
                        ..
                    }
                ))),
                ..
            }]
        ));
        assert!(
            rule_ops(
                &managers,
                &catalog,
                &subscriber,
                &BattleEvent::AllyAction(Default::default())
            )
            .unwrap()
            .is_empty()
        );
    }
}
