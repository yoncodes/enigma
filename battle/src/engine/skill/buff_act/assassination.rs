use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        hp::HurtDamageFromType,
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
    matches!(args, [rate, consume, skill_ids @ ..]
        if *rate > 0 && *consume > 0 && skill_ids.iter().all(|skill_id| *skill_id > 0))
}

pub fn parse_target_trigger(raw_args: &[String]) -> Option<Vec<i32>> {
    let rate = raw_args.first()?.trim().parse::<i32>().ok()?;
    let consume = raw_args.get(1)?.trim().parse::<i32>().ok()?;
    let mut values = vec![rate, consume];
    let mut mapped_values = 0;
    for cell in &raw_args[2..] {
        for part in cell.split(',') {
            let mut atoms = part.split(':');
            let first = atoms.next()?.trim().parse::<i32>().ok()?;
            if first <= 0 {
                return None;
            }
            values.push(first);
            mapped_values += 1;
            if let Some(second) = atoms.next() {
                let second = second.trim().parse::<i32>().ok()?;
                if second <= 0 || atoms.next().is_some() {
                    return None;
                }
                values.push(second);
                mapped_values += 1;
            }
        }
    }
    (rate > 0 && consume > 0 && mapped_values > 0).then_some(values)
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
}
