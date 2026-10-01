use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        hp::{HpCommand, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        buff_act::{feature_command_origin, is_kind, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp},
        target::TargetPool,
    },
};

/// Splits each attack hit on a ShareHurt holder before it lands: one stack is consumed, every
/// other living main ally loses an equal share, and the holder keeps the same share.
pub fn expand(managers: &BattleManagers, pool: &TargetPool, op: &RuleOp) -> Option<Vec<RuleOp>> {
    let commands = match op {
        RuleOp::Command(BattleCommand::Hp(command)) => vec![*command],
        RuleOp::Command(BattleCommand::HpBatch(commands)) => commands.clone(),
        _ => return None,
    };
    // A batch carrying shares was produced here and is already split.
    if commands.iter().any(|command| {
        hurt(command).is_some_and(|hurt| hurt.damage_from == HurtDamageFromType::ShareHurt)
    }) {
        return None;
    }

    let mut expanded = Vec::new();
    let mut unshared = Vec::new();
    let mut split_any = false;
    for command in commands {
        let Some([consume, shared]) = split(managers, pool, command) else {
            unshared.push(command);
            continue;
        };
        if !unshared.is_empty() {
            expanded.push(RuleOp::Command(BattleCommand::HpBatch(std::mem::take(
                &mut unshared,
            ))));
        }
        expanded.extend([consume, shared]);
        split_any = true;
    }
    if !split_any {
        return None;
    }
    if !unshared.is_empty() {
        expanded.push(RuleOp::Command(BattleCommand::HpBatch(unshared)));
    }
    Some(expanded)
}

fn split(managers: &BattleManagers, pool: &TargetPool, command: HpCommand) -> Option<[RuleOp; 2]> {
    let (target_uid, source_uid, amount, config_effect, hurt) = match command {
        HpCommand::Damage(damage) => (
            damage.target_uid,
            damage.source_uid,
            damage.amount,
            damage.config_effect,
            damage.hurt,
        ),
        HpCommand::Lose(loss) => (
            loss.target_uid,
            loss.source_uid,
            loss.amount,
            loss.config_effect,
            loss.hurt?,
        ),
        _ => return None,
    };
    // Only attacks are shared: skill hits and their skill-effect damage.
    if amount <= 0
        || !matches!(
            hurt.damage_from,
            HurtDamageFromType::Skill | HurtDamageFromType::SkillEffect
        )
    {
        return None;
    }
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| {
            feature.owner_uid == target_uid && is_kind(feature, BuffActKind::ShareHurt)
        })?;
    let allies = pool
        .main_allies(target_uid)
        .iter()
        .filter(|ally| ally.uid != target_uid && managers.hp.current(ally.uid) > 0)
        .map(|ally| ally.uid)
        .collect::<Vec<_>>();
    if allies.is_empty() {
        return None;
    }
    let share = amount / (allies.len() as i32 + 1);
    let origin = feature_command_origin(&feature)?;
    let mut shared = allies
        .into_iter()
        .map(|ally_uid| {
            HpCommand::Lose(HpLoss {
                origin,
                source_uid,
                target_uid: ally_uid,
                amount: share,
                config_effect,
                hurt: Some(HurtInfoData {
                    from_uid: hurt.from_uid,
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
    shared.push(with_amount(command, share));
    Some([
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
            origin,
            target_uid,
            selector: BuffSelector::Uid(feature.buff_uid),
            amount: 1,
            depleted: DepletedBuff::Remove,
        }))),
        RuleOp::Command(BattleCommand::HpBatch(shared)),
    ])
}

fn hurt(command: &HpCommand) -> Option<HurtInfoData> {
    match command {
        HpCommand::Damage(damage) => Some(damage.hurt),
        HpCommand::Lose(loss) => loss.hurt,
        _ => None,
    }
}

fn with_amount(mut command: HpCommand, amount: i32) -> HpCommand {
    match &mut command {
        HpCommand::Damage(damage) => damage.amount = amount,
        HpCommand::Lose(loss) => loss.amount = amount,
        _ => {}
    }
    command
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        manager::hp::{DamageEffectKind, HpDamage},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    fn fight(dead: &[i64]) -> Fight {
        let entity = |uid: i64| FightEntityInfo {
            uid: Some(uid),
            current_hp: Some(if dead.contains(&uid) { 0 } else { 10_000 }),
            attr: Some(HeroAttribute {
                hp: Some(10_000),
                ..Default::default()
            }),
            buffs: (uid == 10)
                .then(|| BuffInfo {
                    uid: Some(50),
                    buff_id: Some(31090121),
                    from_uid: Some(13),
                    count: Some(3),
                    ..Default::default()
                })
                .into_iter()
                .collect(),
            ..Default::default()
        };
        Fight {
            attacker: Some(FightTeam {
                entitys: [10, 11, 12, 13].into_iter().map(entity).collect(),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn hit(damage_from: HurtDamageFromType) -> HpCommand {
        HpCommand::Damage(HpDamage {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "SkillDamage"),
            },
            source_uid: -1,
            target_uid: 10,
            amount: 2102,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: -1,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 0,
                skill_id: 1,
                damage_from,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: EffectType::Damage as i32,
                display_amount: None,
            },
        })
    }

    fn amounts(op: &RuleOp) -> Vec<(i64, i32, HurtDamageFromType)> {
        let RuleOp::Command(BattleCommand::HpBatch(commands)) = op else {
            panic!("expected the shared HP batch");
        };
        commands
            .iter()
            .map(|command| match command {
                HpCommand::Damage(damage) => {
                    (damage.target_uid, damage.amount, damage.hurt.damage_from)
                }
                HpCommand::Lose(loss) => (
                    loss.target_uid,
                    loss.amount,
                    loss.hurt
                        .expect("shared losses carry hurt info")
                        .damage_from,
                ),
                _ => panic!("unexpected HP command"),
            })
            .collect()
    }

    #[test]
    fn splits_the_hit_evenly_before_it_lands_and_consumes_one_stack() {
        crate::test_support::init_config();
        let fight = fight(&[]);
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);

        let ops = expand(
            &managers,
            &pool,
            &RuleOp::Command(BattleCommand::Hp(hit(HurtDamageFromType::Skill))),
        )
        .unwrap();

        assert!(matches!(
            &ops[0],
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: 10,
                selector: BuffSelector::Uid(50),
                amount: 1,
                ..
            })))
        ));
        assert_eq!(
            amounts(&ops[1]),
            vec![
                (11, 525, HurtDamageFromType::ShareHurt),
                (12, 525, HurtDamageFromType::ShareHurt),
                (13, 525, HurtDamageFromType::ShareHurt),
                (10, 525, HurtDamageFromType::Skill),
            ]
        );
        assert_eq!(ops.len(), 2);
        assert!(expand(&managers, &pool, &ops[1]).is_none());
    }

    #[test]
    fn dead_allies_are_left_out_of_the_split() {
        crate::test_support::init_config();
        let fight = fight(&[13]);
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);

        let ops = expand(
            &managers,
            &pool,
            &RuleOp::Command(BattleCommand::Hp(hit(HurtDamageFromType::Skill))),
        )
        .unwrap();

        assert_eq!(
            amounts(&ops[1]),
            vec![
                (11, 700, HurtDamageFromType::ShareHurt),
                (12, 700, HurtDamageFromType::ShareHurt),
                (10, 700, HurtDamageFromType::Skill),
            ]
        );
    }

    #[test]
    fn damage_that_is_not_an_attack_is_not_shared() {
        crate::test_support::init_config();
        let fight = fight(&[]);
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);

        assert!(
            expand(
                &managers,
                &pool,
                &RuleOp::Command(BattleCommand::Hp(hit(HurtDamageFromType::Buff))),
            )
            .is_none()
        );
    }
}
