use sonettobuf::CardInfo;

use crate::engine::{
    manager::{
        BattleManagers,
        eureka::{EurekaState, PowerType},
        ex_point::ExPointKind,
    },
    skill::buff_act::{is_kind, registry::BuffActKind},
    skill::target::{TargetEntity, TargetPool},
};

#[derive(Debug, Clone, Default)]
pub struct CardMechanic;

impl CardMechanic {
    pub fn boss_ultimate_power(
        &self,
        managers: &BattleManagers,
        owner_uid: i64,
    ) -> Option<EurekaState> {
        let state = managers
            .eureka
            .get(owner_uid, PowerType::ZongMaoBossEnergy.id());
        (state.max > 0).then_some(state)
    }

    pub fn normal_hand_limit(
        &self,
        base: usize,
        managers: &BattleManagers,
        pool: &TargetPool,
    ) -> usize {
        let modifier = pool
            .attacker_main
            .iter()
            .filter(|entity| managers.hp.current(entity.uid) > 0)
            .map(|entity| {
                managers
                    .buff
                    .buff_act_scalar(entity.uid, BuffActKind::CardLimitAdd)
            })
            .fold(0_i32, i32::saturating_add);
        usize::try_from(
            (base as i32)
                .saturating_add(modifier)
                .saturating_add(managers.card.hand_limit_bonus())
                .max(0),
        )
        .unwrap_or(usize::MAX)
        .max(managers.card.refill_floor())
    }

    pub fn ultimate_cost_offset(&self, managers: &BattleManagers, owner_uid: i64) -> i32 {
        if ExPointKind::from_wire(managers.ex_point.kind(owner_uid)) != ExPointKind::Common {
            return 0;
        }
        managers
            .buff
            .buff_act_scalar(
                owner_uid,
                crate::engine::skill::buff_act::registry::BuffActKind::ExSkillPointChange,
            )
            .saturating_add(managers.buff.buff_act_argument_scalar(
                owner_uid,
                crate::engine::skill::buff_act::registry::BuffActKind::SpExPointMaxAdd,
                1,
            ))
    }

    pub fn required_ultimate_cost(&self, managers: &BattleManagers, entity: &TargetEntity) -> i32 {
        let configured =
            crate::engine::skill::effect::catalog::configured_big_skill_point(entity.ex_skill);
        if ExPointKind::from_wire(managers.ex_point.kind(entity.uid)) != ExPointKind::Common {
            return configured.max(0);
        }
        (if configured > 0 {
            configured
        } else {
            ExPointKind::Common.default_max()
        })
        .saturating_add(self.ultimate_cost_offset(managers, entity.uid))
        .max(0)
    }

    pub fn ultimate_ready(&self, managers: &BattleManagers, entity: &TargetEntity) -> bool {
        if entity.ex_skill <= 0 {
            return false;
        }
        if let Some(power) = self.boss_ultimate_power(managers, entity.uid) {
            return power.is_full()
                && !managers
                    .buff
                    .has_buff_act_kind(entity.uid, BuffActKind::CantGetExskill);
        }
        let kind = ExPointKind::from_wire(managers.ex_point.kind(entity.uid));
        let required = self.required_ultimate_cost(managers, entity);
        let resource_ready = if kind == ExPointKind::Common || required > 0 {
            managers.ex_point.get(entity.uid) >= required
        } else if kind == ExPointKind::Faith {
            managers.ex_point.get(entity.uid) > 0
        } else {
            managers.ex_point.is_full(entity.uid)
        };
        resource_ready
            && !managers
                .buff
                .has_buff_act_kind(entity.uid, BuffActKind::CantGetExskill)
    }

    pub fn is_ultimate(&self, card: &CardInfo, entity: &TargetEntity) -> bool {
        card.uid == Some(entity.uid)
            && card
                .skill_id
                .is_some_and(|skill_id| self.is_ultimate_skill(skill_id, entity))
    }

    pub fn is_ultimate_skill(&self, skill_id: i32, entity: &TargetEntity) -> bool {
        skill_id == entity.ex_skill
            || config::try_get()
                .and_then(|db| db.skill.get(skill_id))
                .is_some_and(|skill| {
                    skill.hero_id == entity.model_id
                        && crate::engine::skill::effect::catalog::configured_is_big_skill(skill_id)
                })
    }

    pub fn can_add_normal_ultimate(
        &self,
        managers: &BattleManagers,
        entity: &TargetEntity,
    ) -> bool {
        self.ultimate_ready(managers, entity)
            && !self.ultimate_ignores_limit(managers, entity.uid, entity.ex_skill)
            && !managers
                .card
                .hand()
                .iter()
                .chain(managers.card.team_cards())
                .any(|card| self.is_ultimate(card, entity))
    }

    pub fn refill_hand_len(&self, managers: &BattleManagers, pool: &TargetPool) -> usize {
        managers
            .card
            .hand()
            .iter()
            .filter(|card| self.counts_toward_hand_limit(card, managers, pool))
            .count()
    }

    pub fn counts_toward_hand_limit(
        &self,
        card: &CardInfo,
        managers: &BattleManagers,
        pool: &TargetPool,
    ) -> bool {
        !card.temp_card.unwrap_or_default()
            && pool
                .entity(card.uid.unwrap_or_default())
                .is_none_or(|entity| {
                    !self.is_ultimate(card, entity)
                        || !self.ultimate_ignores_limit(
                            managers,
                            entity.uid,
                            card.skill_id.unwrap_or_default(),
                        )
                })
    }

    pub fn is_device_card(&self, card: &CardInfo) -> bool {
        card.skill_id.is_some_and(|skill_id| {
            crate::engine::skill::effect::catalog::configured_effect_tag(skill_id)
                == crate::engine::skill::effect::catalog::SkillEffectTag::Device as i32
        })
    }

    pub fn normal_ultimate_cards(
        &self,
        pool: &TargetPool,
        managers: &BattleManagers,
    ) -> Vec<CardInfo> {
        pool.attacker_main
            .iter()
            .filter(|entity| managers.hp.current(entity.uid) > 0)
            .filter(|entity| self.can_add_normal_ultimate(managers, entity))
            .filter_map(|entity| {
                managers
                    .card
                    .draw_pile()
                    .iter()
                    .find(|card| self.is_ultimate(card, entity))
                    .cloned()
                    .or_else(|| {
                        crate::engine::manager::card::pool::card_for_target(entity, entity.ex_skill)
                    })
            })
            .collect()
    }

    pub fn ultimate_ignores_limit(
        &self,
        managers: &BattleManagers,
        owner_uid: i64,
        skill_id: i32,
    ) -> bool {
        managers
            .buff
            .active_features(&managers.hp)
            .iter()
            .any(|feature| {
                feature.owner_uid == owner_uid
                    && ((is_kind(feature, BuffActKind::CardNotCalSize)
                        && feature.values.contains(&skill_id))
                        || is_kind(feature, BuffActKind::EntityExSkillNotCalSize))
            })
    }

    pub fn special_team_cards(
        &self,
        pool: &TargetPool,
        managers: &BattleManagers,
        before_hand: &[CardInfo],
    ) -> Vec<CardInfo> {
        pool.attacker_main
            .iter()
            .filter(|entity| managers.hp.current(entity.uid) > 0)
            .filter_map(|entity| {
                let uid = entity.uid;
                let skill_id = entity.ex_skill;
                (self.ultimate_ready(managers, entity)
                    && !before_hand
                        .iter()
                        .any(|card| self.is_ultimate(card, entity))
                    && self.ultimate_ignores_limit(managers, uid, skill_id))
                .then(|| crate::engine::manager::card::pool::card_for_target(entity, skill_id))
                .flatten()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;

    fn fight(ex_point: i32) -> Fight {
        Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3139),
                    current_hp: Some(100),
                    ex_point: Some(ex_point),
                    ex_skill: Some(31390131),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31390181),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    #[test]
    fn card_not_cal_size_adds_a_ready_ultimate_beyond_the_normal_hand() {
        crate::test_support::init_config();
        let mut fight = fight(5);
        fight
            .attacker
            .as_mut()
            .unwrap()
            .entitys
            .push(FightEntityInfo {
                uid: Some(11),
                model_id: Some(3134),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_skill: Some(31340131),
                buffs: vec![BuffInfo {
                    uid: Some(21),
                    buff_id: Some(31390183),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            });
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let hand = (0..9)
            .map(|skill_id| CardInfo {
                uid: Some(10),
                skill_id: Some(skill_id + 1),
                ..Default::default()
            })
            .collect::<Vec<_>>();

        let extra = CardMechanic.special_team_cards(&pool, &managers, &hand);

        assert_eq!(hand.len() + extra.len(), 11);
        assert_eq!(
            extra
                .iter()
                .filter_map(|card| card.skill_id)
                .collect::<Vec<_>>(),
            vec![31390131, 31340131]
        );
    }

    #[test]
    fn entity_ultimate_not_cal_size_is_scoped_to_its_buff_owner() {
        crate::test_support::init_config();
        let mut fight = fight(5);
        fight.attacker.as_mut().unwrap().entitys[0].buffs = vec![BuffInfo {
            uid: Some(20),
            buff_id: Some(31390190),
            from_uid: Some(10),
            ..Default::default()
        }];
        let managers = BattleManagers::seeded(&fight);
        let mechanic = CardMechanic;

        assert!(mechanic.ultimate_ignores_limit(&managers, 10, 31390131));
        assert!(!mechanic.ultimate_ignores_limit(&managers, 11, 31340131));
    }

    #[test]
    fn card_not_cal_size_does_not_duplicate_or_unlock_an_ultimate() {
        crate::test_support::init_config();
        let locked = fight(4);
        let locked_pool = TargetPool::from_fight(&locked);
        assert!(
            CardMechanic
                .special_team_cards(&locked_pool, &BattleManagers::seeded(&locked), &[])
                .is_empty()
        );

        let ready = fight(5);
        let ready_pool = TargetPool::from_fight(&ready);
        let ultimate = CardInfo {
            uid: Some(10),
            skill_id: Some(31390131),
            ..Default::default()
        };
        assert!(
            CardMechanic
                .special_team_cards(&ready_pool, &BattleManagers::seeded(&ready), &[ultimate],)
                .is_empty()
        );
    }

    #[test]
    fn configured_ultimate_rank_alias_blocks_a_duplicate() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3134),
                    current_hp: Some(100),
                    ex_point: Some(5),
                    ex_skill: Some(31345131),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let card = CardInfo {
            uid: Some(10),
            skill_id: Some(31340131),
            ..Default::default()
        };

        assert!(CardMechanic.is_ultimate(&card, &pool.attacker_main[0]));
        assert!(
            CardMechanic
                .special_team_cards(&pool, &BattleManagers::seeded(&fight), &[card])
                .is_empty()
        );
    }

    #[test]
    fn faith_channel_is_ready_with_positive_faith_instead_of_a_full_gauge() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3120),
                    current_hp: Some(100),
                    ex_point: Some(1),
                    ex_point_type: Some(ExPointKind::Faith.as_wire()),
                    ex_skill: Some(31200133),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let managers = BattleManagers::seeded(&fight);

        assert_eq!(
            CardMechanic
                .normal_ultimate_cards(&pool, &managers)
                .iter()
                .filter_map(|card| card.skill_id)
                .collect::<Vec<_>>(),
            vec![31200133]
        );
    }

    #[test]
    fn active_ultimate_cost_buff_controls_card_readiness() {
        crate::test_support::init_config();
        let mut fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(222001),
                    current_hp: Some(100),
                    ex_point: Some(1),
                    ex_skill: Some(222001231),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(2220012),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let managers = BattleManagers::seeded(&fight);

        assert_eq!(
            CardMechanic.required_ultimate_cost(&managers, &pool.attacker_main[0]),
            1
        );
        assert_eq!(
            CardMechanic
                .normal_ultimate_cards(&pool, &managers)
                .iter()
                .filter_map(|card| card.skill_id)
                .collect::<Vec<_>>(),
            vec![222001231]
        );
        managers.sync_entities(&mut fight);
        assert_eq!(
            fight.attacker.unwrap().entitys[0].ex_skill_point_change,
            Some(-4)
        );
    }

    #[test]
    fn configured_buff_acts_adjust_the_normal_hand_limit() {
        crate::test_support::init_config();
        let fight_with = |buff_id| Fight {
            attacker: Some(FightTeam {
                entitys: (0..4)
                    .map(|index| FightEntityInfo {
                        uid: Some(index + 1),
                        model_id: Some(1000 + index as i32),
                        current_hp: Some(100),
                        buffs: (index == 0)
                            .then(|| BuffInfo {
                                uid: Some(20),
                                buff_id: Some(buff_id),
                                from_uid: Some(1),
                                ..Default::default()
                            })
                            .into_iter()
                            .collect(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        };

        for (buff_id, expected) in [(31490001, 10), (31370131, 6)] {
            let fight = fight_with(buff_id);
            let managers = BattleManagers::seeded(&fight);
            let pool = TargetPool::from_fight(&fight);

            assert_eq!(
                CardMechanic.normal_hand_limit(8, &managers, &pool),
                expected
            );
        }
    }
}
