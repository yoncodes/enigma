use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffGrant, BuffSelector, DepletedBuff},
    },
    skill::{
        buff_act::registry::BuffActKind,
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AssassinationModifier {
    pub assassinate: bool,
    pub triggered_by_target: bool,
    pub final_damage_bonus: i32,
}

pub fn supports_source_bonus(args: &[i32]) -> bool {
    matches!(args, [rate] if *rate > 0)
}

pub fn supports_target_trigger(args: &[i32]) -> bool {
    matches!(args, [rate, consume, mappings @ ..]
        if *rate > 0
            && *consume > 0
            && !mappings.is_empty()
            && mappings.len() % 2 == 0
            && mappings.iter().all(|skill_id| *skill_id > 0))
}

pub fn parse_target_trigger(raw_args: &[String]) -> Option<Vec<i32>> {
    let [rate, consume, mappings] = raw_args else {
        return None;
    };
    let rate = rate.trim().parse::<i32>().ok()?;
    let consume = consume.trim().parse::<i32>().ok()?;
    let mut values = vec![rate, consume];
    for pair in mappings.split(':') {
        let mut pair = pair.split(',');
        let passive_skill = pair.next()?;
        let active_skill = pair.next()?;
        if pair.next().is_some() {
            return None;
        }
        for skill_id in [passive_skill, active_skill] {
            let skill_id = skill_id.trim().parse::<i32>().ok()?;
            if skill_id <= 0 {
                return None;
            }
            values.push(skill_id);
        }
    }
    supports_target_trigger(&values).then_some(values)
}

// Targets marked when an action begins, with each mark's damage rate. Marks the action inflicts
// itself land after it attacked, and a mark consumed as the attack starts still counts for it.
pub fn marked_targets(managers: &BattleManagers) -> Vec<(i64, i32)> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.amount > 0)
        .filter(|feature| super::is_kind(feature, BuffActKind::BeAttackedAssassinate))
        .filter_map(|feature| Some((feature.owner_uid, *feature.values.get(1)?)))
        .collect()
}

pub fn target_modifier(
    managers: &BattleManagers,
    source_uid: i64,
    target_uid: i64,
    already_assassinate: bool,
    marks_at_action_start: Option<&[(i64, i32)]>,
) -> AssassinationModifier {
    let features = managers.buff.active_features(&managers.hp);
    let live_marks;
    let marks = match marks_at_action_start {
        Some(marks) => marks,
        None => {
            live_marks = marked_targets(managers);
            &live_marks
        }
    };
    let (marked, target_rate) = marks
        .iter()
        .filter(|(owner_uid, _)| *owner_uid == target_uid)
        .fold((false, 0), |(_, rate), (_, mark_rate)| {
            (true, rate.max(*mark_rate))
        });
    let assassinate = already_assassinate || marked;
    let source_rate = features
        .iter()
        .filter(|feature| feature.owner_alive && feature.owner_uid == source_uid)
        .filter(|feature| super::is_kind(feature, BuffActKind::AddAssassinateY))
        .filter_map(|feature| feature.values.get(1))
        .copied()
        .sum::<i32>();
    let technique_excess = (managers.origin_attribute(source_uid, AttrId::CriticalTechnique)
        - managers.origin_attribute(target_uid, AttrId::CriticalTechnique))
    .max(0);
    AssassinationModifier {
        assassinate,
        triggered_by_target: marked && !already_assassinate,
        final_damage_bonus: i32::from(assassinate)
            * (technique_excess / 100)
            * (target_rate + source_rate),
    }
}

pub fn rule_ops(
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::BeAttackedAssassinate) {
        return None;
    }
    let BattleEvent::SkillAction(action) = event else {
        return Some(Vec::new());
    };
    if action.phase != crate::engine::skill::action::SkillPhase::Immediate {
        return Some(Vec::new());
    }
    // "When being attacked, trigger [Assassination] and remove 1 stack", as the attack starts.
    if let [_, amount, _, ..] = subscriber.args.as_slice() {
        if !action.is_attack
            || catalog.damage_rate(action.skill_id) <= 0
            || action.source_uid == subscriber.owner_uid
            || !action.marked_targets.contains(&subscriber.owner_uid)
            || !action.target_uids.contains(&subscriber.owner_uid)
            || catalog.is_assassinate(action.skill_id)
        {
            return Some(Vec::new());
        }
        return Some(vec![RuleOp::Command(BattleCommand::Buff(
            BuffCommand::Consume(BuffConsume {
                origin: super::command_origin(subscriber)?,
                target_uid: subscriber.owner_uid,
                selector: BuffSelector::Uid(subscriber.buff_uid),
                amount: *amount,
                depleted: DepletedBuff::Remove,
            }),
        ))]);
    }
    {
        let [active_skill_id] = subscriber.args.as_slice() else {
            return None;
        };
        if action.source_uid != subscriber.owner_uid || action.skill_id != *active_skill_id {
            return Some(Vec::new());
        }
        let mut targets = action.target_uids.clone();
        if targets.is_empty() && action.target_uid != 0 {
            targets.push(action.target_uid);
        }
        targets.retain(|target_uid| *target_uid != 0);
        targets.sort_unstable();
        targets.dedup();
        return Some(
            targets
                .into_iter()
                .map(|target_uid| {
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                        origin: super::command_origin(subscriber).expect("registered buff act"),
                        source_uid: subscriber.owner_uid,
                        target_uid,
                        buff_id: subscriber.buff_id,
                        amount: Some(1),
                        occurrences: 1,
                        child_uid_reservations: 0,
                    })))
                })
                .collect(),
        );
    }
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;

    #[test]
    fn lethal_injury_marks_the_attack_and_scales_final_damage_from_technique_excess() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        technic: Some(450),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(19),
                        buff_id: Some(2295033),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        technic: Some(120),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31240121),
                        from_uid: Some(10),
                        layer: Some(3),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        let modifier = target_modifier(&BattleManagers::seeded(&fight), 10, -1, false, None);
        // The attack consumed the last stack as it started; the start-of-action mark still counts.
        let mut consumed = fight.clone();
        consumed.defender.as_mut().unwrap().entitys[0].buffs.clear();
        let consumed = BattleManagers::seeded(&consumed);

        let expected = AssassinationModifier {
            assassinate: true,
            triggered_by_target: true,
            final_damage_bonus: 282,
        };
        assert_eq!(modifier, expected);
        assert_eq!(
            target_modifier(&consumed, 10, -1, false, Some(&[(-1, 10)])),
            expected
        );
        assert!(
            !target_modifier(&BattleManagers::seeded(&fight), 10, -1, false, Some(&[])).assassinate
        );
    }

    #[test]
    fn only_damaging_attacks_on_a_target_marked_at_action_start_consume_lethal_injury() {
        crate::test_support::init_config();
        let catalog =
            SkillEffectCatalog::from_roots(config::configs::get(), [312431212, 435221], []);
        let subscriber = BuffActSubscriber {
            owner_uid: -1,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 31240121,
            team_type: 2,
            owner_alive: true,
            amount: 1,
            key: crate::engine::event::subscription::SubscriptionKey::new(
                crate::engine::event::kind::EventKind::SkillAction,
                crate::engine::skill::rule::DefinitionKey::new(10004, "BeAttackedAssassinate"),
            ),
            act_type: "BeAttackedAssassinate".to_owned(),
            effect_time: 202,
            effect_condition: 0,
            args: vec![10, 1, 312401451, 31240121],
            raw: "10004#10#1#312401451,31240121".to_owned(),
        };
        let attack = |skill_id, marked_targets: Vec<i64>| {
            BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
                source_uid: 11,
                skill_id,
                target_uid: -1,
                target_uids: vec![-1],
                attacked_target_uids: Vec::new(),
                phase: crate::engine::skill::action::SkillPhase::Immediate,
                skill_slot: 0,
                is_attack: true,
                rank: 1,
                skill_type: 1,
                effect_tag: 1,
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
                marked_targets,
            })
        };
        let consumes = |event| {
            rule_ops(&catalog, &subscriber, &event)
                .unwrap()
                .iter()
                .any(|op| {
                    matches!(
                        op,
                        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(_)))
                    )
                })
        };

        assert!(consumes(attack(312431212, vec![-1])));
        assert!(!consumes(attack(312431212, Vec::new())));
        assert!(!consumes(attack(435221, vec![-1])));
    }

    #[test]
    fn independent_attacker_bonuses_add_for_an_inherent_assassination() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        technic: Some(450),
                        ..Default::default()
                    }),
                    buffs: vec![
                        BuffInfo {
                            uid: Some(19),
                            buff_id: Some(312451460),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                        BuffInfo {
                            uid: Some(20),
                            buff_id: Some(435211),
                            from_uid: Some(10),
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        technic: Some(120),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(
            target_modifier(&BattleManagers::seeded(&fight), 10, -1, true, None),
            AssassinationModifier {
                assassinate: true,
                triggered_by_target: false,
                final_damage_bonus: 150,
            }
        );
    }

    #[test]
    fn active_field_stack_grants_are_unique_per_target_and_commit_through_buff_manager() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    passive_skill: vec![312401453, 312401453],
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(312401453),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(-1),
                        current_hp: Some(100),
                        buffs: vec![BuffInfo {
                            uid: Some(30),
                            buff_id: Some(31240121),
                            from_uid: Some(10),
                            layer: Some(1),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(-2),
                        current_hp: Some(100),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
        let catalog =
            SkillEffectCatalog::from_roots(config::configs::get(), [31240103, 312401453], []);
        let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
            source_uid: 10,
            skill_id: 31240103,
            target_uid: -1,
            target_uids: vec![-1, -1, -2],
            attacked_target_uids: vec![-1, -2],
            phase: crate::engine::skill::action::SkillPhase::Immediate,
            skill_slot: 0,
            is_attack: true,
            rank: 1,
            skill_type: 1,
            effect_tag: 1,
            assassinate: true,
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
            marked_targets: Vec::new(),
        });
        let mapped_ops = |managers: &BattleManagers| {
            crate::engine::event::dispatcher::dispatch_event(
                &pool,
                managers,
                &catalog,
                &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
                &event,
            )
            .unwrap()
            .buff_acts
            .into_iter()
            .flat_map(|(_, ops)| ops.unwrap_or_default())
            .collect::<Vec<_>>()
        };
        let mut without_field = fight.clone();
        without_field.attacker.as_mut().unwrap().entitys[0]
            .buffs
            .clear();
        assert!(mapped_ops(&BattleManagers::seeded(&without_field)).is_empty());
        let ops = mapped_ops(&managers);

        assert_eq!(ops.len(), 2);
        for op in ops {
            let RuleOp::Command(BattleCommand::Buff(command @ BuffCommand::Grant(_))) = op.op
            else {
                panic!("expected a normal buff grant");
            };
            managers.execute_buff(command).unwrap();
        }
        assert_eq!(managers.buff.max_id_or_type_layer(-1, 31240121), 2);
        assert_eq!(managers.buff.max_id_or_type_layer(-2, 31240121), 1);
    }
}
