use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffGrant, BuffSelector, DepletedBuff},
    },
    skill::{
        behavior::classify::BehaviorKind,
        buff_act::registry::BuffActKind,
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

// Assassination grants Final DMG +5% per 100 excess Critical Technique. A target mark's first
// reaction can convert the attack; its configured rate applies to later reactions in that action.
const TARGET_ASSASSINATION_BASE_RATE: i32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetMark {
    pub owner_uid: i64,
    pub buff_id: i32,
    pub rate: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsupportedReactionCardinality {
    pub skill_id: i32,
    pub buff_id: i32,
    pub count: usize,
}

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
pub fn marked_targets(managers: &BattleManagers) -> Vec<TargetMark> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.amount > 0)
        .filter(|feature| super::is_kind(feature, BuffActKind::BeAttackedAssassinate))
        .filter_map(|feature| {
            Some(TargetMark {
                owner_uid: feature.owner_uid,
                buff_id: feature.buff_id,
                rate: *feature.values.get(1)?,
            })
        })
        .collect()
}

fn configured_target_trigger_count(
    catalog: &SkillEffectCatalog,
    skill_id: i32,
    buff_id: i32,
) -> usize {
    catalog
        .get(skill_id)
        .map(|effect| {
            effect
                .slots
                .iter()
                .filter(|slot| {
                    slot.behavior.spec.kind == BehaviorKind::AddBuff
                        && slot.behavior.arg(0) == Some(buff_id)
                })
                .count()
        })
        .unwrap_or_default()
}

fn target_rate_applies(
    skill_id: i32,
    buff_id: i32,
    inherent: bool,
    count: usize,
) -> Result<bool, UnsupportedReactionCardinality> {
    if !inherent && count > 2 {
        return Err(UnsupportedReactionCardinality {
            skill_id,
            buff_id,
            count,
        });
    }
    Ok(inherent || count == 2)
}

pub fn rate_marks_for_skill(
    marks: &[TargetMark],
    catalog: &SkillEffectCatalog,
    skill_id: i32,
) -> Result<Vec<(i64, i32)>, UnsupportedReactionCardinality> {
    let inherent = catalog.is_assassinate(skill_id);
    marks
        .iter()
        .filter_map(|mark| {
            let count = configured_target_trigger_count(catalog, skill_id, mark.buff_id);
            match target_rate_applies(skill_id, mark.buff_id, inherent, count) {
                Ok(true) => Some(Ok((mark.owner_uid, mark.rate))),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect()
}

pub fn target_modifier(
    managers: &BattleManagers,
    source_uid: i64,
    target_uid: i64,
    already_assassinate: bool,
    marks_at_action_start: Option<&[TargetMark]>,
    rate_marks_at_action_start: Option<&[(i64, i32)]>,
) -> AssassinationModifier {
    let features = managers.buff.active_features(&managers.hp);
    let live_marks = marked_targets(managers);
    let marks = match marks_at_action_start {
        Some(marks) => marks,
        None => &live_marks,
    };
    let marked = marks.iter().any(|mark| mark.owner_uid == target_uid);
    let target_rate = match rate_marks_at_action_start {
        Some(marks) => marks
            .iter()
            .filter(|(owner_uid, _)| *owner_uid == target_uid)
            .map(|(_, mark_rate)| *mark_rate)
            .max()
            .unwrap_or_default(),
        None if already_assassinate => live_marks
            .iter()
            .filter(|mark| mark.owner_uid == target_uid)
            .map(|mark| mark.rate)
            .max()
            .unwrap_or_default(),
        None => 0,
    };
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
            * (TARGET_ASSASSINATION_BASE_RATE + source_rate + target_rate),
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
    let Some(action) = event.skill_action_event() else {
        return Some(Vec::new());
    };
    if action.phase != crate::engine::skill::action::SkillPhase::Immediate {
        return Some(Vec::new());
    }
    // "When being attacked, trigger [Assassination] and remove 1 stack", before the attack's own effects.
    if let [_, amount, _, ..] = subscriber.args.as_slice() {
        if !matches!(event, BattleEvent::SkillEffectStarted(_)) {
            return Some(Vec::new());
        }
        if !action.is_attack
            || catalog.damage_rate(action.skill_id) <= 0
            || action.source_uid == subscriber.owner_uid
            || subscriber.amount <= 0
            || !action.target_uids.contains(&subscriber.owner_uid)
            || catalog.is_assassinate(action.skill_id)
        {
            return Some(Vec::new());
        }
        // Separate configured mark applications each trigger this reaction; their layer amounts do not.
        let reaction_count =
            configured_target_trigger_count(catalog, action.skill_id, subscriber.buff_id).max(1);
        return Some(vec![RuleOp::Command(BattleCommand::Buff(
            BuffCommand::Consume(BuffConsume {
                origin: super::command_origin(subscriber)?,
                target_uid: subscriber.owner_uid,
                selector: BuffSelector::Uid(subscriber.buff_uid),
                amount: amount.saturating_mul(i32::try_from(reaction_count).unwrap_or(i32::MAX)),
                depleted: DepletedBuff::RemoveWithZeroLayer,
            }),
        ))]);
    }
    let [active_skill_id] = subscriber.args.as_slice() else {
        return None;
    };
    if !matches!(event, BattleEvent::SkillAction(_))
        || action.source_uid != subscriber.owner_uid
        || action.skill_id != *active_skill_id
    {
        return Some(Vec::new());
    }
    let mut targets = action.target_uids.clone();
    if targets.is_empty() && action.target_uid != 0 {
        targets.push(action.target_uid);
    }
    targets.retain(|target_uid| *target_uid != 0);
    targets.sort_unstable();
    targets.dedup();
    Some(
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
    )
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

        let modifier = target_modifier(&BattleManagers::seeded(&fight), 10, -1, false, None, None);
        // The attack consumed the last stack as it started; the start-of-action mark still counts.
        let mut consumed = fight.clone();
        consumed.defender.as_mut().unwrap().entitys[0].buffs.clear();
        let consumed = BattleManagers::seeded(&consumed);

        let triggered = AssassinationModifier {
            assassinate: true,
            triggered_by_target: true,
            final_damage_bonus: 402,
        };
        let start_mark = TargetMark {
            owner_uid: -1,
            buff_id: 31240121,
            rate: 10,
        };
        assert_eq!(modifier, triggered);
        assert_eq!(
            target_modifier(
                &consumed,
                10,
                -1,
                false,
                Some(&[start_mark]),
                Some(&[(-1, 10)]),
            ),
            AssassinationModifier {
                final_damage_bonus: 432,
                ..triggered
            }
        );
        assert_eq!(
            target_modifier(
                &BattleManagers::seeded(&fight),
                10,
                -1,
                false,
                Some(&[start_mark]),
                Some(&[]),
            ),
            AssassinationModifier {
                final_damage_bonus: 402,
                ..triggered
            }
        );
        assert_eq!(
            target_modifier(&BattleManagers::seeded(&fight), 10, -1, true, None, None,),
            AssassinationModifier {
                assassinate: true,
                triggered_by_target: false,
                final_damage_bonus: 432,
            }
        );
        assert!(!target_modifier(&consumed, 10, -1, false, Some(&[]), Some(&[])).assassinate);
    }

    #[test]
    fn lethal_injury_rate_uses_exact_configured_reaction_cardinality() {
        crate::test_support::init_config();
        let fight = Fight {
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
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
        let managers = BattleManagers::seeded(&fight);
        let marks = marked_targets(&managers);
        let catalog = SkillEffectCatalog::from_roots(
            config::configs::get(),
            [31243121, 312431212, 312401214, 312451031],
            [],
        );

        assert_eq!(rate_marks_for_skill(&marks, &catalog, 31243121), Ok(vec![]));
        assert_eq!(
            rate_marks_for_skill(&marks, &catalog, 312431212),
            Ok(vec![(-1, 10)])
        );
        assert_eq!(
            rate_marks_for_skill(&marks, &catalog, 312401214),
            Ok(vec![(-1, 10)])
        );
        assert_eq!(
            rate_marks_for_skill(&marks, &catalog, 312451031),
            Ok(vec![(-1, 10)])
        );
        assert_eq!(
            target_rate_applies(999, 888, false, 3),
            Err(UnsupportedReactionCardinality {
                skill_id: 999,
                buff_id: 888,
                count: 3,
            })
        );
    }

    #[test]
    fn only_damaging_attacks_on_a_marked_target_consume_lethal_injury_as_they_start() {
        crate::test_support::init_config();
        let catalog = SkillEffectCatalog::from_roots(
            config::configs::get(),
            [31243122, 312431212, 312431222, 435221],
            [],
        );
        let lethal_injury = |stacks| BuffActSubscriber {
            owner_uid: -1,
            source_uid: 10,
            buff_uid: 20,
            buff_id: 31240121,
            team_type: 2,
            owner_alive: true,
            amount: stacks,
            key: crate::engine::event::subscription::SubscriptionKey::new(
                crate::engine::event::kind::EventKind::SkillEffectStarted,
                crate::engine::skill::rule::DefinitionKey::new(10004, "BeAttackedAssassinate"),
            ),
            act_type: "BeAttackedAssassinate".to_owned(),
            effect_time: 202,
            effect_condition: 0,
            args: vec![10, 1, 312401451, 31240121],
            raw: "10004#10#1#312401451,31240121".to_owned(),
        };
        let action = |skill_id, rank| crate::engine::skill::action::SkillActionEvent {
            source_uid: 11,
            skill_id,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: Vec::new(),
            phase: crate::engine::skill::action::SkillPhase::Immediate,
            skill_slot: 0,
            is_attack: true,
            rank,
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
        };
        let consume = |stacks, event| {
            rule_ops(&catalog, &lethal_injury(stacks), &event)
                .unwrap()
                .into_iter()
                .find_map(|op| match op {
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(consume))) => {
                        Some(consume)
                    }
                    _ => None,
                })
        };
        let starts =
            |skill_id, rank| BattleEvent::SkillEffectStarted(action(skill_id, rank).into());

        assert_eq!(
            consume(1, starts(312431212, 1)).unwrap().depleted,
            DepletedBuff::RemoveWithZeroLayer
        );
        assert_eq!(consume(3, starts(312431212, 1)).unwrap().amount, 2);
        assert_eq!(consume(2, starts(312431222, 2)).unwrap().amount, 2);
        assert_eq!(consume(2, starts(31243122, 2)).unwrap().amount, 1);
        assert!(consume(0, starts(312431212, 1)).is_none());
        assert!(consume(1, starts(435221, 1)).is_none());
        // Not after the attack's own start effects.
        assert!(consume(1, BattleEvent::SkillAction(action(312431212, 1))).is_none());
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
            target_modifier(&BattleManagers::seeded(&fight), 10, -1, true, None, None,),
            AssassinationModifier {
                assassinate: true,
                triggered_by_target: false,
                final_damage_bonus: 300,
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
