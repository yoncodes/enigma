use crate::engine::{
    damage::{scale_permille, scale_permille_stacks},
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{
            ActiveBuffFeature, BuffAccumulateActValue, BuffCommand, BuffRemove, BuffRemoveSelector,
            BuffSetState,
        },
        hp::{HpCommand, HpHeal, HpHealKind, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        buff_act::{self, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InjuryBankState {
    pub current: i32,
    pub cap: i32,
}

pub fn rule_op(managers: &BattleManagers, subscriber: &BuffActSubscriber) -> Option<RuleOp> {
    if !buff_act::subscriber_is_kind(subscriber, BuffActKind::InjuryBank) {
        return None;
    }
    let [raw_attr_id, permille, ..] = subscriber.args.as_slice() else {
        return None;
    };
    if AttrId::from_raw(*raw_attr_id) != Some(AttrId::Hp) {
        return None;
    }
    let cap = scale_permille_stacks(
        managers.hp.max(subscriber.owner_uid),
        *permille,
        subscriber.amount,
    );
    Some(RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(
        BuffSetState {
            ex_info: None,
            origin: buff_act::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            params: Some(format!(
                "{}#0#{}",
                subscriber.key.definition.opcode,
                cap.max(0)
            )),
            act_info: None,
        },
    ))))
}

pub fn grant_rule_op(managers: &BattleManagers, feature: &ActiveBuffFeature) -> Option<RuleOp> {
    if !buff_act::is_kind(feature, BuffActKind::InjuryBank) {
        return None;
    }
    let [_, raw_attr_id, permille, ..] = feature.values.as_slice() else {
        return None;
    };
    if AttrId::from_raw(*raw_attr_id) != Some(AttrId::Hp) {
        return None;
    }
    let cap = scale_permille_stacks(
        managers.hp.max(feature.owner_uid),
        *permille,
        feature.amount,
    );
    Some(RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(
        BuffSetState {
            ex_info: None,
            origin: buff_act::feature_command_origin(feature)?,
            target_uid: feature.owner_uid,
            buff_uid: feature.buff_uid,
            params: Some(format!("{}#0#{}", feature.act_id()?, cap.max(0))),
            act_info: None,
        },
    ))))
}

pub fn grant_transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    super::changed_features(managers, event, BuffActKind::InjuryBank)
        .into_iter()
        .filter_map(|(feature, _)| grant_rule_op(managers, &feature).map(|op| (feature, op)))
        .collect()
}

pub fn runtime_rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !buff_act::subscriber_is_kind(subscriber, BuffActKind::InjuryBank) {
        return None;
    }
    let BattleEvent::HpLost {
        target_uid, amount, ..
    } = event
    else {
        return Some(Vec::new());
    };
    if *target_uid != subscriber.owner_uid {
        return Some(Vec::new());
    }
    let [
        raw_attr_id,
        cap_permille,
        _heal_skill_id,
        threshold_permille,
        heal_permille,
        store_permille,
    ] = subscriber.args.as_slice()
    else {
        return None;
    };
    if AttrId::from_raw(*raw_attr_id) != Some(AttrId::Hp)
        || *cap_permille <= 0
        || *threshold_permille <= 0
        || *heal_permille <= 0
        || *store_permille <= 0
    {
        return None;
    }
    let max_hp = managers.hp.max(subscriber.owner_uid).max(0);
    let state = state(managers, subscriber.owner_uid).unwrap_or(InjuryBankState {
        current: 0,
        cap: scale_permille(max_hp, *cap_permille),
    });
    let stored = scale_permille(*amount, *store_permille).min((state.cap - state.current).max(0));
    if stored <= 0 {
        return Some(Vec::new());
    }
    let next = state.current + stored;
    let threshold = scale_permille(max_hp, *threshold_permille);
    let heals = if threshold > 0 {
        next / threshold - state.current / threshold
    } else {
        0
    };
    let origin = buff_act::command_origin(subscriber)?;
    let mut ops = vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(BuffSetState {
            ex_info: None,
            origin,
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            params: Some(format!(
                "{}#{}#{}",
                subscriber.key.definition.opcode, next, state.cap
            )),
            act_info: None,
        }))),
        RuleOp::BuffFeatureMarker {
            target_uid: subscriber.owner_uid,
            effect_type: *buff_act::wire::find(
                subscriber.key.definition.opcode,
                &subscriber.act_type,
            )?
            .markers(buff_act::wire::WirePhase::Refresh)
            .first()?,
            effect_num: stored,
            buff_act_id: 0,
        },
    ];
    let heal = scale_permille(max_hp, *heal_permille);
    for _ in 0..heals {
        ops.extend(
            pool.allies(subscriber.owner_uid)
                .iter()
                .filter(|ally| managers.hp.current(ally.uid) > 0)
                .map(|ally| {
                    RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(HpHeal {
                        origin,
                        source_uid: subscriber.owner_uid,
                        target_uid: ally.uid,
                        amount: heal,
                        config_effect: subscriber.key.definition.opcode,
                        kind: HpHealKind::Normal,
                    })))
                }),
        );
    }
    Some(ops)
}

pub fn logback_rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !buff_act::subscriber_is_kind(subscriber, BuffActKind::InjuryLogback) {
        return None;
    }
    let [settle_permille] = subscriber.args.as_slice() else {
        return None;
    };
    if *settle_permille <= 0 {
        return None;
    }
    let origin = buff_act::command_origin(subscriber)?;
    match event {
        BattleEvent::HpLost {
            target_uid,
            amount,
            buff_uid,
            ..
        } if *target_uid == subscriber.owner_uid
            && *amount > 0
            && *buff_uid != Some(subscriber.buff_uid) =>
        {
            Some(vec![RuleOp::Command(BattleCommand::Buff(
                BuffCommand::AccumulateActValue(BuffAccumulateActValue {
                    origin,
                    target_uid: subscriber.owner_uid,
                    buff_uid: subscriber.buff_uid,
                    act_id: subscriber.key.definition.opcode,
                    delta: *amount,
                }),
            ))])
        }
        BattleEvent::Kind(crate::engine::event::kind::EventKind::RoundEndFinalSettlement) => {
            let recorded = managers
                .buff
                .act_value(subscriber.buff_uid, subscriber.key.definition.opcode);
            let amount = scale_permille(recorded, *settle_permille);
            let mut ops = Vec::with_capacity(2);
            if amount > 0 {
                ops.push(RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(
                    HpLoss {
                        origin,
                        source_uid: subscriber.source_uid,
                        target_uid: subscriber.owner_uid,
                        amount,
                        config_effect: 0,
                        hurt: Some(HurtInfoData {
                            from_uid: subscriber.source_uid,
                            is_crit: false,
                            career_restraint: false,
                            reduce_hp: 0,
                            effect_id: 0,
                            skill_id: 0,
                            damage_from: HurtDamageFromType::Buff,
                            buff_act_id: subscriber.key.definition.opcode,
                            buff_uid: subscriber.buff_uid,
                            hurt_effect_type: sonettobuf::effect_type_enum::EffectType::Origindamage
                                as i32,
                            display_amount: None,
                        }),
                    },
                ))));
            }
            ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
                BuffRemove {
                    origin,
                    target_uid: subscriber.owner_uid,
                    selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
                },
            ))));
            Some(ops)
        }
        _ => Some(Vec::new()),
    }
}

pub fn state(managers: &BattleManagers, owner_uid: i64) -> Option<InjuryBankState> {
    feature_state(managers, owner_uid).map(|(_, state)| state)
}

pub fn feature_state(
    managers: &BattleManagers,
    owner_uid: i64,
) -> Option<(ActiveBuffFeature, InjuryBankState)> {
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| {
            feature.owner_uid == owner_uid && buff_act::is_kind(feature, BuffActKind::InjuryBank)
        })?;
    let state = managers
        .buff
        .snapshot(owner_uid, feature.buff_uid)
        .and_then(|buff| parse_state(buff.act_common_params.as_deref()))?;
    Some((feature, state))
}

pub fn snapshotted_attribute_delta(
    feature: &ActiveBuffFeature,
    attr_id: AttrId,
    buffs: &crate::engine::manager::buff::BuffManager,
) -> i32 {
    if !buff_act::is_kind(feature, BuffActKind::AttrFixFromInjuryBank) {
        return 0;
    }
    let [act_id, raw_attr_id, ..] = feature.values.as_slice() else {
        return 0;
    };
    if AttrId::from_raw(*raw_attr_id) != Some(attr_id) {
        return 0;
    }
    let Some(raw) = buffs
        .snapshot(feature.owner_uid, feature.buff_uid)
        .and_then(|buff| buff.act_common_params)
    else {
        return 0;
    };
    let mut parts = raw.split('#');
    if parts.next().and_then(|part| part.parse::<i32>().ok()) != Some(*act_id) {
        return 0;
    }
    parts
        .next()
        .and_then(|part| part.parse::<i32>().ok())
        .unwrap_or_default()
}

fn parse_state(raw: Option<&str>) -> Option<InjuryBankState> {
    let mut parts = raw?.split('#');
    (parts.next()?.parse::<i32>().ok()? == 770).then_some(())?;
    let current = parts.next()?.parse().ok()?;
    let cap = parts.next()?.parse().ok()?;
    Some(InjuryBankState { current, cap })
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        event::{bus::EventBus, kind::EventKind, subscription::SubscriptionKey},
        runtime::executor::execute_rule_op,
    };

    #[test]
    fn injury_bank_cap_is_a_buff_command_not_private_scanner_state() {
        crate::test_support::init_config();
        let fight = Fight {
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
                        buff_id: Some(30800141),
                        from_uid: Some(10),
                        act_common_params: Some("770#0".to_owned()),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30800141,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::BuffAdded,
                crate::engine::skill::rule::DefinitionKey::new(770, "InjuryBank"),
            ),
            act_type: "InjuryBank".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            args: vec![101, 200, 30800161, 30, 100, 100],
            raw: "770#101#200#30800161#30#100#100".to_owned(),
        };
        let output = rule_op(&managers, &subscriber).unwrap();

        execute_rule_op(&mut managers, &mut EventBus::default(), output).unwrap();

        assert_eq!(
            managers
                .buff
                .snapshot(10, 20)
                .unwrap()
                .act_common_params
                .as_deref(),
            Some("770#0#200")
        );
    }

    #[test]
    fn stored_injury_crossing_a_threshold_heals_each_living_ally() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        team_type: Some(1),
                        current_hp: Some(800),
                        attr: Some(HeroAttribute {
                            hp: Some(1_000),
                            ..Default::default()
                        }),
                        buffs: vec![BuffInfo {
                            uid: Some(20),
                            buff_id: Some(30800141),
                            from_uid: Some(10),
                            act_common_params: Some("770#20#200".to_owned()),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(11),
                        team_type: Some(1),
                        current_hp: Some(500),
                        attr: Some(HeroAttribute {
                            hp: Some(1_000),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 30800141,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::HpLost,
                crate::engine::skill::rule::DefinitionKey::new(770, "InjuryBank"),
            ),
            act_type: "InjuryBank".to_owned(),
            effect_time: 0,
            effect_condition: 0,
            args: vec![101, 200, 30800161, 30, 100, 100],
            raw: "770#101#200#30800161#30#100#100".to_owned(),
        };
        let event = BattleEvent::HpLost {
            origin: crate::engine::skill::rule::CommandOrigin {
                domain: crate::engine::skill::rule::RuleDomain::Behavior,
                key: crate::engine::skill::rule::DefinitionKey::new(1, "Damage"),
            },
            source_uid: -1,
            skill_id: 1,
            target_uid: 10,
            amount: 100,
            buff_uid: None,
        };

        let ops = runtime_rule_ops(&managers, &pool, &subscriber, &event).unwrap();

        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(BuffSetState {
                    params: Some(params),
                    ..
                }))),
                RuleOp::BuffFeatureMarker { effect_num: 10, .. },
                RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(HpHeal {
                    target_uid: 10,
                    amount: 100,
                    ..
                }))),
                RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(HpHeal {
                    target_uid: 11,
                    amount: 100,
                    ..
                })))
            ] if params == "770#30#200"
        ));
    }

    #[test]
    fn attribute_delta_reads_the_grant_snapshot_as_a_flat_value() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(30800111),
                        from_uid: Some(11),
                        act_common_params: Some("767#317".to_owned()),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let feature = managers
            .buff
            .active_features(&managers.hp)
            .into_iter()
            .find(|feature| buff_act::is_kind(feature, BuffActKind::AttrFixFromInjuryBank))
            .unwrap();

        assert_eq!(
            snapshotted_attribute_delta(&feature, AttrId::Attack, &managers.buff),
            317
        );
    }
}
