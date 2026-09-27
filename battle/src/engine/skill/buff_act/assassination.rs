use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffGrant, BuffSelector, DepletedBuff},
        hp::HurtDamageFromType,
    },
    skill::{
        buff_act::registry::BuffActKind,
        effect::SkillEffectCatalog,
        rule::{
            CommandOrigin, RuleDomain,
            output::{BattleCommand, RuleOp},
        },
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

pub fn mapped_stack_rule_ops(
    catalog: &SkillEffectCatalog,
    managers: &BattleManagers,
    source_uid: i64,
    active_skill_id: i32,
    target_uids: &[i64],
) -> Vec<RuleOp> {
    let Some(passive_skills) = managers.entity.passive_skills(source_uid) else {
        return Vec::new();
    };
    let mut grants = Vec::new();
    for passive_skill_id in passive_skills {
        for &(buff_id, key) in
            catalog.assassination_stack_grants(*passive_skill_id, active_skill_id)
        {
            for target_uid in target_uids
                .iter()
                .copied()
                .filter(|target_uid| *target_uid != 0)
            {
                if grants
                    .iter()
                    .any(|&(target, buff, _)| target == target_uid && buff == buff_id)
                {
                    continue;
                }
                grants.push((target_uid, buff_id, key));
            }
        }
    }
    grants
        .into_iter()
        .map(|(target_uid, buff_id, key)| {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key,
                },
                source_uid,
                target_uid,
                buff_id,
                amount: Some(1),
                occurrences: 1,
                child_uid_reservations: 0,
            })))
        })
        .collect()
}

pub fn target_modifier(
    managers: &BattleManagers,
    source_uid: i64,
    target_uid: i64,
    already_assassinate: bool,
) -> AssassinationModifier {
    let features = managers.buff.active_features(&managers.hp);
    let mut target_rate = 0;
    let mut marked = false;
    for feature in features
        .iter()
        .filter(|feature| feature.owner_uid == target_uid && feature.amount > 0)
        .filter(|feature| super::is_kind(feature, BuffActKind::BeAttackedAssassinate))
    {
        let [_, configured_per_hundred, ..] = feature.values.as_slice() else {
            continue;
        };
        marked = true;
        target_rate = target_rate.max(*configured_per_hundred);
    }
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
    let BattleEvent::Hit(hit) = event else {
        return Some(Vec::new());
    };
    if hit.target_uid != subscriber.owner_uid
        || hit.amount <= 0
        || hit.damage_from != HurtDamageFromType::Skill
        || !hit.assassinate
        || catalog.is_assassinate(hit.skill_id)
    {
        return Some(Vec::new());
    }
    let [_, amount, ..] = subscriber.args.as_slice() else {
        return None;
    };
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::Consume(BuffConsume {
            origin: super::command_origin(subscriber)?,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::Uid(subscriber.buff_uid),
            amount: *amount,
            depleted: DepletedBuff::Remove,
        }),
    ))])
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

        let modifier = target_modifier(&BattleManagers::seeded(&fight), 10, -1, false);

        assert_eq!(
            modifier,
            AssassinationModifier {
                assassinate: true,
                triggered_by_target: true,
                final_damage_bonus: 282,
            }
        );
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
            target_modifier(&BattleManagers::seeded(&fight), 10, -1, true),
            AssassinationModifier {
                assassinate: true,
                triggered_by_target: false,
                final_damage_bonus: 150,
            }
        );
    }

    #[test]
    fn mapped_stack_grants_are_unique_per_target_and_commit_through_buff_manager() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    passive_skill: vec![312401453, 312401453],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(-1),
                        current_hp: Some(100),
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
        let catalog =
            SkillEffectCatalog::from_roots(config::configs::get(), [31240103, 312401453], []);
        let mut managers = BattleManagers::seeded(&fight);
        let ops = mapped_stack_rule_ops(&catalog, &managers, 10, 31240103, &[-1, -1, -2]);

        assert_eq!(ops.len(), 2);
        for op in ops {
            let RuleOp::Command(BattleCommand::Buff(command @ BuffCommand::Grant(_))) = op else {
                panic!("expected a normal buff grant");
            };
            managers.execute_buff(command).unwrap();
        }
        assert_eq!(managers.buff.max_id_or_type_layer(-1, 31240121), 1);
        assert_eq!(managers.buff.max_id_or_type_layer(-2, 31240121), 1);
    }
}
