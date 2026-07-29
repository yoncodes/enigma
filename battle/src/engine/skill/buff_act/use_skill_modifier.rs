use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        action::{SkillExecutionMode, SkillPhase},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports_loss(args: &[i32]) -> bool {
    matches!(args, [rate] if *rate > 0)
}

pub fn supports_attribute(args: &[i32]) -> bool {
    matches!(args, [raw_attr, value]
        if AttrId::from_raw(*raw_attr).is_some() && *value != 0)
}

fn is_active_cast(mode: SkillExecutionMode) -> bool {
    matches!(
        mode,
        SkillExecutionMode::Active | SkillExecutionMode::DirectBig
    )
}

pub fn loss_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::SkillAction(action) = event else {
        return None;
    };
    let [rate] = subscriber.args.as_slice() else {
        return None;
    };
    if !supports_loss(&subscriber.args)
        || action.source_uid != subscriber.owner_uid
        || action.phase != SkillPhase::Immediate
        || !is_active_cast(action.mode)
        || !subscriber.owner_alive
    {
        return Some(Vec::new());
    }
    let amount = managers.hp.current(subscriber.owner_uid).max(0) * *rate / 1000;
    if amount <= 0 {
        return Some(Vec::new());
    }
    let origin = super::command_origin(subscriber)?;
    Some(vec![RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
        HpLoss {
            origin,
            source_uid: subscriber.owner_uid,
            target_uid: subscriber.owner_uid,
            amount,
            config_effect: 0,
            hurt: Some(HurtInfoData {
                from_uid: subscriber.source_uid,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: action.skill_id,
                skill_id: action.skill_id,
                damage_from: HurtDamageFromType::Buff,
                buff_act_id: subscriber.key.definition.opcode,
                buff_uid: subscriber.buff_uid,
                hurt_effect_type: 0,
                display_amount: None,
            }),
        },
    )))])
}

pub fn attribute_deltas(
    managers: &BattleManagers,
    owner_uid: i64,
    mode: SkillExecutionMode,
) -> Vec<(AttrId, i32)> {
    if !is_active_cast(mode) {
        return Vec::new();
    }
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == owner_uid
                && super::is_kind(feature, super::registry::BuffActKind::UseSkillAttrFix)
        })
        .filter_map(|feature| {
            let [_, raw_attr, value] = feature.values.as_slice() else {
                return None;
            };
            supports_attribute(&[*raw_attr, *value])
                .then(|| AttrId::from_raw(*raw_attr).map(|attr| (attr, *value)))
                .flatten()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::{
            action::SkillActionEvent,
            rule::{DefinitionKey, output::BattleCommand},
        },
    };

    fn managers() -> BattleManagers {
        crate::test_support::init_config();
        BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(30880121),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    fn subscriber() -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30880121,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::SkillAction,
                DefinitionKey::new(757, "UseSkillLoseHpNotFixed"),
            ),
            act_type: "UseSkillLoseHpNotFixed".to_owned(),
            effect_time: 201,
            effect_condition: 0,
            args: vec![50],
            raw: "757#50".to_owned(),
        }
    }

    fn event(mode: SkillExecutionMode) -> BattleEvent {
        BattleEvent::SkillAction(SkillActionEvent {
            source_uid: 10,
            skill_id: 1,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase: SkillPhase::Immediate,
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            skill_type: 0,
            effect_tag: 0,
            assassinate: false,
            damage_amount: 0,
            kill_count: 0,
            crit_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
            buff_additions: Vec::new(),
        })
    }

    #[test]
    fn active_cast_loses_current_hp_but_nested_skill_does_not() {
        let managers = managers();
        let subscriber = subscriber();

        assert!(matches!(
            loss_rule_ops(&managers, &subscriber, &event(SkillExecutionMode::Active))
                .unwrap()
                .as_slice(),
            [RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
                HpLoss { amount: 50, .. }
            )))]
        ));
        assert!(
            loss_rule_ops(&managers, &subscriber, &event(SkillExecutionMode::Nested))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn active_cast_reads_all_configured_action_attributes() {
        let managers = managers();

        assert_eq!(
            attribute_deltas(&managers, 10, SkillExecutionMode::Active),
            vec![(AttrId::CriticalRate, 150)]
        );
        assert!(attribute_deltas(&managers, 10, SkillExecutionMode::Nested).is_empty());
    }
}
