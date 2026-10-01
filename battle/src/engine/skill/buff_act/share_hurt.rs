use std::collections::HashMap;

use sonettobuf::effect_type_enum::EffectType;

use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffConsume, BuffManager, BuffSelector, DepletedBuff},
        entity::EntityManager,
        hp::{HpDamage, HpLoss, HpManager, HurtDamageFromType, HurtInfoData},
    },
    skill::buff_act::{feature_command_origin, is_kind, registry::BuffActKind},
};

/// How one attack hit on a ShareHurt holder is split before it lands.
#[derive(Debug, Clone, PartialEq)]
pub struct SharePlan {
    pub buff_uid: i64,
    pub consume: BuffCommand,
    pub shares: Vec<HpLoss>,
    pub holder_amount: i32,
}

/// Plans the split of an attack hit on a ShareHurt holder: one stack is consumed, every other
/// living main ally loses an equal share, and the holder keeps the same share. `spent` counts
/// stacks already planned for consumption earlier in the same HP batch.
pub fn plan(
    buffs: &BuffManager,
    hp: &HpManager,
    entities: &EntityManager,
    damage: &HpDamage,
    spent: &HashMap<i64, i32>,
) -> Option<SharePlan> {
    // Captures prove the split for skill hits and their skill-effect damage only.
    let hurt = damage.hurt;
    if damage.amount <= 0
        || !matches!(
            hurt.damage_from,
            HurtDamageFromType::Skill | HurtDamageFromType::SkillEffect
        )
    {
        return None;
    }
    let feature = buffs.active_features(hp).into_iter().find(|feature| {
        feature.owner_uid == damage.target_uid
            && is_kind(feature, BuffActKind::ShareHurt)
            && feature.amount > spent.get(&feature.buff_uid).copied().unwrap_or_default()
    })?;
    let allies = entities
        .alive_combatants(entities.team_type(damage.target_uid)?, hp)
        .into_iter()
        .filter(|uid| *uid != damage.target_uid)
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
        .map(|ally_uid| HpLoss {
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
        .collect();
    Some(SharePlan {
        buff_uid: feature.buff_uid,
        consume: BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: damage.target_uid,
            selector: BuffSelector::Uid(feature.buff_uid),
            // Consume plans store the resulting layer, so count stacks planned earlier in the batch.
            amount: 1 + spent.get(&feature.buff_uid).copied().unwrap_or_default(),
            depleted: DepletedBuff::Remove,
        }),
        shares,
        holder_amount: share,
    })
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use crate::engine::{
        event::payload::BattleEvent,
        manager::{
            BattleManagers,
            hp::{
                DamageEffectKind, HpChanges, HpCommand, HpDamage, HpLoss, HurtDamageFromType,
                HurtInfoData,
            },
        },
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    fn managers(dead: &[i64], stacks: i32) -> BattleManagers {
        crate::test_support::init_config();
        let entity = |(position, uid): (i32, i64)| FightEntityInfo {
            uid: Some(uid),
            position: Some(position),
            team_type: Some(1),
            current_hp: Some(if dead.contains(&uid) {
                0
            } else if uid == 11 && stacks < 0 {
                100
            } else {
                10_000
            }),
            attr: Some(HeroAttribute {
                hp: Some(10_000),
                ..Default::default()
            }),
            buffs: (uid == 10)
                .then(|| BuffInfo {
                    uid: Some(50),
                    buff_id: Some(31090121),
                    from_uid: Some(13),
                    layer: Some(stacks.abs()),
                    ..Default::default()
                })
                .into_iter()
                .collect(),
            ..Default::default()
        };
        BattleManagers::seeded(&Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: [(1, 10), (2, 11), (3, 12), (4, 13)]
                    .into_iter()
                    .map(entity)
                    .collect(),
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    position: Some(1),
                    team_type: Some(2),
                    current_hp: Some(10_000),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    fn hit(target_uid: i64, damage_from: HurtDamageFromType) -> HpDamage {
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
                hurt_effect_type: sonettobuf::effect_type_enum::EffectType::Damage as i32,
                display_amount: None,
            },
        }
    }

    fn shares(changes: &HpChanges) -> Vec<(i64, i32)> {
        changes
            .shared_hurt
            .as_ref()
            .map(|shared| {
                shared
                    .shares
                    .iter()
                    .map(|share| {
                        (
                            share.target_uid,
                            share.hp.map(|hp| -hp.delta).unwrap_or_default(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    #[test]
    fn a_hit_on_the_holder_is_split_before_it_lands() {
        let mut managers = managers(&[], 3);

        let changes = managers
            .execute_hp(HpCommand::Damage(hit(10, HurtDamageFromType::Skill)))
            .unwrap();

        assert_eq!(shares(&changes), vec![(11, 525), (12, 525), (13, 525)]);
        assert!(managers.buff.has_buff_id(10, 31090121));
        for uid in [10, 11, 12, 13] {
            assert_eq!(managers.hp.current(uid), 10_000 - 525);
        }
    }

    #[test]
    fn each_share_publishes_the_attacker_and_the_sharing_ally() {
        let mut managers = managers(&[], 3);

        let changes = managers
            .execute_hp(HpCommand::Damage(hit(10, HurtDamageFromType::Skill)))
            .unwrap();

        let shared = changes
            .events()
            .into_iter()
            .filter_map(|event| match event {
                BattleEvent::DamageShared {
                    source_uid,
                    target_uid,
                    amount,
                    ..
                } => Some((source_uid, target_uid, amount)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(shared, vec![(-1, 11, 525), (-1, 12, 525), (-1, 13, 525)]);
    }

    #[test]
    fn dead_allies_are_left_out_of_the_split() {
        let mut managers = managers(&[13], 3);

        let changes = managers
            .execute_hp(HpCommand::Damage(hit(10, HurtDamageFromType::Skill)))
            .unwrap();

        assert_eq!(shares(&changes), vec![(11, 700), (12, 700)]);
        assert_eq!(managers.hp.current(10), 10_000 - 700);
    }

    #[test]
    fn a_holder_spent_mid_batch_takes_the_next_hit_unsplit() {
        let mut managers = managers(&[], 1);

        let batch = managers
            .execute_hp_batch(vec![
                HpCommand::Damage(hit(10, HurtDamageFromType::Skill)),
                HpCommand::Damage(hit(10, HurtDamageFromType::Skill)),
            ])
            .unwrap();

        assert_eq!(batch.len(), 2);
        assert_eq!(shares(&batch[0]).len(), 3);
        assert!(batch[1].shared_hurt.is_none());
        assert!(!managers.buff.has_buff_id(10, 31090121));
        assert_eq!(managers.hp.current(10), 10_000 - 525 - 2102);
        assert_eq!(managers.hp.current(11), 10_000 - 525);
    }

    #[test]
    fn a_mixed_batch_stays_one_attack() {
        let mut managers = managers(&[], 3);

        let batch = managers
            .execute_hp_batch(vec![
                HpCommand::Damage(hit(11, HurtDamageFromType::Skill)),
                HpCommand::Damage(hit(10, HurtDamageFromType::Skill)),
            ])
            .unwrap();

        assert_eq!(batch.len(), 2);
        assert!(batch[0].shared_hurt.is_none());
        assert_eq!(shares(&batch[1]), vec![(11, 525), (12, 525), (13, 525)]);
        assert_eq!(managers.hp.current(11), 10_000 - 2102 - 525);
    }

    #[test]
    fn skill_effect_shares_keep_their_config_effect_and_ids() {
        let mut managers = managers(&[], 3);
        let mut genesis = hit(10, HurtDamageFromType::SkillEffect);
        genesis.config_effect = 30014;
        genesis.hurt.effect_id = 109380001;
        genesis.hurt.skill_id = 109380001;

        let changes = managers.execute_hp(HpCommand::Damage(genesis)).unwrap();

        let share = &changes.shared_hurt.as_ref().unwrap().shares[0];
        let hurt = share.hp.and_then(|hp| hp.hurt).unwrap();
        assert_eq!(
            (hurt.damage_from, hurt.effect_id, hurt.skill_id),
            (HurtDamageFromType::ShareHurt, 109380001, 109380001)
        );
    }

    #[test]
    fn hp_losses_and_non_attack_damage_are_not_split() {
        let mut managers = managers(&[], 3);
        let damage = hit(10, HurtDamageFromType::Skill);

        let lost = managers
            .execute_hp(HpCommand::Lose(HpLoss {
                origin: damage.origin,
                source_uid: damage.source_uid,
                target_uid: 10,
                amount: 100,
                config_effect: -1,
                hurt: Some(damage.hurt),
            }))
            .unwrap();
        let burned = managers
            .execute_hp(HpCommand::Damage(hit(10, HurtDamageFromType::Buff)))
            .unwrap();

        assert!(lost.shared_hurt.is_none() && burned.shared_hurt.is_none());
        assert_eq!(managers.hp.current(11), 10_000);
    }

    #[test]
    fn two_hits_in_one_batch_spend_two_stacks() {
        let mut managers = managers(&[], 2);

        let batch = managers
            .execute_hp_batch(vec![
                HpCommand::Damage(hit(10, HurtDamageFromType::Skill)),
                HpCommand::Damage(hit(10, HurtDamageFromType::Skill)),
            ])
            .unwrap();

        assert!(batch.iter().all(|changes| changes.shared_hurt.is_some()));
        assert!(!managers.buff.has_buff_id(10, 31090121));
        assert_eq!(managers.hp.current(11), 10_000 - 525 - 525);
    }

    #[test]
    fn an_ally_killed_by_a_share_reaches_death_settlement() {
        // A negative stack count marks ally 11 as nearly dead in this fixture.
        let mut managers = managers(&[], -3);

        let execution = managers
            .execute_rule_hp(HpCommand::Damage(hit(10, HurtDamageFromType::Skill)))
            .unwrap();
        let mut outcome = crate::engine::runtime::executor::RuleOutcome::Hp(Box::new(execution));

        assert_eq!(outcome.death_count(), 1);
        assert!(outcome.injured_targets().contains(&11));
        let deaths = outcome.take_deaths();
        assert_eq!(
            deaths
                .iter()
                .map(|death| death.target_uid)
                .collect::<Vec<_>>(),
            vec![11]
        );
    }
}
