use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
        hp::{HpCommand, HpDamage, HpLoss, HurtDamageFromType, HurtInfoData},
    },
    skill::{
        buff_act::{
            feature_command_origin, is_kind,
            registry::{BuffActKind, InterceptedHpOp},
        },
        rule::output::{BattleCommand, RuleOp},
        target::TargetPool,
    },
};

/// Splits the first attack hit on a ShareHurt holder before it lands: one stack is consumed,
/// every other living main ally loses an equal share, then the holder's hit lands with the
/// same share in its original op shape. Later hits in a batch are re-queued so each split sees
/// the committed state.
pub fn expand(
    managers: &BattleManagers,
    pool: &TargetPool,
    op: &RuleOp,
) -> Option<Vec<InterceptedHpOp>> {
    let (commands, batched) = match op {
        RuleOp::Command(BattleCommand::Hp(command)) => (vec![*command], false),
        RuleOp::Command(BattleCommand::HpBatch(commands)) => (commands.clone(), true),
        _ => return None,
    };
    let (index, split) = commands
        .iter()
        .enumerate()
        .find_map(|(index, command)| Some((index, split(managers, pool, *command)?)))?;
    let pending = |op| InterceptedHpOp { op, settled: false };
    let mut expanded = Vec::with_capacity(5);
    if index > 0 {
        expanded.push(pending(hp_batch(commands[..index].to_vec())));
    }
    expanded.push(pending(split.consume));
    expanded.push(pending(hp_batch(split.shares)));
    expanded.push(InterceptedHpOp {
        op: if batched {
            hp_batch(vec![split.hit])
        } else {
            RuleOp::Command(BattleCommand::Hp(split.hit))
        },
        settled: true,
    });
    if index + 1 < commands.len() {
        expanded.push(pending(hp_batch(commands[index + 1..].to_vec())));
    }
    Some(expanded)
}

struct Split {
    consume: RuleOp,
    shares: Vec<HpCommand>,
    hit: HpCommand,
}

fn hp_batch(commands: Vec<HpCommand>) -> RuleOp {
    RuleOp::Command(BattleCommand::HpBatch(commands))
}

fn split(managers: &BattleManagers, pool: &TargetPool, command: HpCommand) -> Option<Split> {
    // Captures prove the split for skill hits and their skill-effect damage only.
    let HpCommand::Damage(damage) = command else {
        return None;
    };
    let hurt = damage.hurt;
    if damage.amount <= 0
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
            feature.owner_uid == damage.target_uid && is_kind(feature, BuffActKind::ShareHurt)
        })?;
    let allies = pool
        .main_allies(damage.target_uid)
        .iter()
        .filter(|ally| ally.uid != damage.target_uid && managers.hp.current(ally.uid) > 0)
        .map(|ally| ally.uid)
        .collect::<Vec<_>>();
    if allies.is_empty() {
        return None;
    }
    let share = damage.amount / (allies.len() as i32 + 1);
    let origin = feature_command_origin(&feature)?;
    // Skill hits share with zero effect and skill ids; skill-effect damage keeps its own.
    let (effect_id, skill_id) = if hurt.damage_from == HurtDamageFromType::SkillEffect {
        (hurt.effect_id, hurt.skill_id)
    } else {
        (0, 0)
    };
    let shares = allies
        .into_iter()
        .map(|ally_uid| {
            HpCommand::Lose(HpLoss {
                origin,
                source_uid: damage.source_uid,
                target_uid: ally_uid,
                amount: share,
                config_effect: damage.config_effect,
                hurt: Some(HurtInfoData {
                    from_uid: hurt.from_uid,
                    is_crit: false,
                    career_restraint: false,
                    reduce_hp: 0,
                    effect_id,
                    skill_id,
                    damage_from: HurtDamageFromType::ShareHurt,
                    buff_act_id: 0,
                    buff_uid: 0,
                    hurt_effect_type: EffectType::Sharehurt as i32,
                    display_amount: None,
                }),
            })
        })
        .collect::<Vec<_>>();
    Some(Split {
        consume: RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: damage.target_uid,
            selector: BuffSelector::Uid(feature.buff_uid),
            amount: 1,
            depleted: DepletedBuff::Remove,
        }))),
        shares,
        hit: HpCommand::Damage(HpDamage {
            amount: share,
            ..damage
        }),
    })
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::{
        manager::hp::DamageEffectKind,
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    pub(crate) fn fight(dead: &[i64], stacks: i32) -> Fight {
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
                    count: Some(stacks),
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

    pub(crate) fn hit(target_uid: i64, damage_from: HurtDamageFromType) -> HpDamage {
        HpDamage {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "SkillDamage"),
            },
            source_uid: -1,
            target_uid,
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
        }
    }

    fn setup(dead: &[i64]) -> (BattleManagers, TargetPool) {
        crate::test_support::init_config();
        let fight = fight(dead, 3);
        (
            BattleManagers::seeded(&fight),
            TargetPool::from_fight(&fight),
        )
    }

    fn share_amounts(op: &RuleOp) -> Vec<(i64, i32)> {
        let RuleOp::Command(BattleCommand::HpBatch(commands)) = op else {
            panic!("expected the shares batch");
        };
        commands
            .iter()
            .map(|command| match command {
                HpCommand::Lose(loss) => {
                    assert_eq!(
                        loss.hurt.map(|hurt| hurt.damage_from),
                        Some(HurtDamageFromType::ShareHurt)
                    );
                    (loss.target_uid, loss.amount)
                }
                _ => panic!("shares are HP losses"),
            })
            .collect()
    }

    fn single(target_uid: i64, damage_from: HurtDamageFromType) -> RuleOp {
        RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(hit(
            target_uid,
            damage_from,
        ))))
    }

    #[test]
    fn a_single_hit_splits_before_it_lands_and_keeps_its_op_shape() {
        let (managers, pool) = setup(&[]);

        let ops = expand(&managers, &pool, &single(10, HurtDamageFromType::Skill)).unwrap();

        assert_eq!(ops.len(), 3);
        assert!(matches!(
            &ops[0].op,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                target_uid: 10,
                selector: BuffSelector::Uid(50),
                amount: 1,
                ..
            })))
        ));
        assert_eq!(
            share_amounts(&ops[1].op),
            vec![(11, 525), (12, 525), (13, 525)]
        );
        assert!(matches!(
            ops[2],
            InterceptedHpOp {
                op: RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(HpDamage {
                    target_uid: 10,
                    amount: 525,
                    ..
                }))),
                settled: true,
            }
        ));
        assert!(!ops[0].settled && !ops[1].settled);
    }

    #[test]
    fn dead_allies_are_left_out_of_the_split() {
        let (managers, pool) = setup(&[13]);

        let ops = expand(&managers, &pool, &single(10, HurtDamageFromType::Skill)).unwrap();

        assert_eq!(share_amounts(&ops[1].op), vec![(11, 700), (12, 700)]);
    }

    #[test]
    fn only_the_first_holder_in_a_batch_splits_and_the_rest_is_requeued() {
        let (managers, pool) = setup(&[]);
        let other = HpCommand::Damage(hit(11, HurtDamageFromType::Skill));

        let ops = expand(
            &managers,
            &pool,
            &RuleOp::Command(BattleCommand::HpBatch(vec![
                other,
                HpCommand::Damage(hit(10, HurtDamageFromType::Skill)),
                other,
            ])),
        )
        .unwrap();

        assert_eq!(ops.len(), 5);
        assert_eq!(
            ops[0].op,
            RuleOp::Command(BattleCommand::HpBatch(vec![other]))
        );
        let RuleOp::Command(BattleCommand::HpBatch(holder)) = &ops[3].op else {
            panic!("a batched holder hit stays batched");
        };
        assert!(ops[3].settled);
        assert!(matches!(
            holder.as_slice(),
            [HpCommand::Damage(HpDamage {
                target_uid: 10,
                amount: 525,
                ..
            })]
        ));
        assert_eq!(
            ops[4],
            InterceptedHpOp {
                op: RuleOp::Command(BattleCommand::HpBatch(vec![other])),
                settled: false,
            }
        );
    }

    #[test]
    fn skill_effect_shares_keep_their_config_effect_and_ids() {
        let (managers, pool) = setup(&[]);
        let mut genesis = hit(10, HurtDamageFromType::SkillEffect);
        genesis.config_effect = 30014;
        genesis.hurt.effect_id = 109380001;
        genesis.hurt.skill_id = 109380001;

        let ops = expand(
            &managers,
            &pool,
            &RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(genesis))),
        )
        .unwrap();

        let RuleOp::Command(BattleCommand::HpBatch(shares)) = &ops[1].op else {
            panic!("expected the shares batch");
        };
        assert!(matches!(
            shares[0],
            HpCommand::Lose(HpLoss {
                config_effect: 30014,
                hurt: Some(HurtInfoData {
                    effect_id: 109380001,
                    skill_id: 109380001,
                    ..
                }),
                ..
            })
        ));
    }

    #[test]
    fn hp_losses_and_non_attack_damage_are_not_split() {
        let (managers, pool) = setup(&[]);
        let damage = hit(10, HurtDamageFromType::SkillEffect);

        assert!(
            expand(
                &managers,
                &pool,
                &RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(HpLoss {
                    origin: damage.origin,
                    source_uid: damage.source_uid,
                    target_uid: damage.target_uid,
                    amount: damage.amount,
                    config_effect: damage.config_effect,
                    hurt: Some(damage.hurt),
                }))),
            )
            .is_none()
        );
        assert!(expand(&managers, &pool, &single(10, HurtDamageFromType::Buff)).is_none());
    }
}
