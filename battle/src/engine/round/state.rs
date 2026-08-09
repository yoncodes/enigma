use sonettobuf::{CardInfo, Fight, FightHeroSpAttributeInfo, FightRound};

use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        buff_act,
        effect::SkillEffectCatalog,
        target::{TargetContext, TargetPool},
    },
};

use super::power::ClothPower;

pub const FIRST_ACTION_ROUND: i32 = 2;

#[derive(Debug, Clone, Default)]
pub struct RoundState {
    pub act_point: i32,
    pub move_num: i32,
    pub is_finish: bool,
    pub cur_round: i32,
    pub power: i32,
    pub before_cards2: Vec<CardInfo>,
    pub team_a_cards2: Vec<CardInfo>,
    pub hero_sp_attributes: Vec<FightHeroSpAttributeInfo>,
    pub last_change_hero_uid: Option<i64>,
}

impl RoundState {
    pub fn start(game_data: &config::GameDB, fight: &Fight) -> Self {
        Self::from_power(fight, ClothPower::initial(game_data, fight))
    }

    pub(crate) fn seeded(catalog: crate::catalog::BattleCatalog, fight: &Fight) -> Self {
        Self::from_power(fight, ClothPower::seeded(catalog, fight))
    }

    fn from_power(fight: &Fight, power: i32) -> Self {
        Self {
            act_point: attacker_main_count(fight),
            move_num: 0,
            is_finish: fight.is_finish.unwrap_or(false),
            cur_round: fight.cur_round.unwrap_or(1),
            power,
            last_change_hero_uid: fight.last_change_hero_uid,
            ..Default::default()
        }
    }

    pub fn begin_round(&mut self) {
        self.cur_round += 1;
    }
}

pub fn attacker_main_count(fight: &Fight) -> i32 {
    fight
        .attacker
        .as_ref()
        .map(|team| team.entitys.len() as i32)
        .unwrap_or(3)
}

pub fn next_action_points(
    fight: &Fight,
    pool: &TargetPool,
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> i32 {
    let alive_attackers = fight
        .attacker
        .as_ref()
        .map(|team| {
            team.entitys
                .iter()
                .filter(|entity| entity.uid.is_some_and(|uid| managers.hp.current(uid) > 0))
                .count() as i32
        })
        .unwrap_or(3);
    let buff_bonus = managers
        .buff
        .active_features(&managers.hp)
        .iter()
        .filter(|feature| feature.team_type == 1)
        .map(buff_act::add_action_point::bonus)
        .sum::<i32>();
    let rule_bonus =
        super::modifier::action_point_bonus(pool, managers, catalog, determinism, context);

    alive_attackers + buff_bonus + rule_bonus
}

pub fn next_round_shell(
    fight: &Fight,
    state: &RoundState,
    include_card_snapshots: bool,
    current_hand: &[CardInfo],
    current_team_cards: &[CardInfo],
    ai_queue: &[CardInfo],
) -> FightRound {
    let attacker = fight.attacker.as_ref();
    let before_cards1 = if include_card_snapshots {
        round_field_cards(current_hand)
    } else {
        Vec::new()
    };
    let team_a_cards1 = if include_card_snapshots {
        round_field_cards(current_team_cards)
    } else {
        Vec::new()
    };

    FightRound {
        act_point: Some(state.act_point),
        is_finish: Some(state.is_finish),
        move_num: Some(state.move_num),
        ai_use_cards: ai_queue.to_vec(),
        power: Some(state.power),
        skill_infos: attacker
            .map(|team| team.skill_infos.clone())
            .unwrap_or_default(),
        before_cards1,
        team_a_cards1,
        before_cards2: if include_card_snapshots {
            state.before_cards2.clone()
        } else {
            Vec::new()
        },
        team_a_cards2: if include_card_snapshots {
            state.team_a_cards2.clone()
        } else {
            Vec::new()
        },
        next_round_begin_step: Vec::new(),
        use_card_list: Vec::new(),
        cur_round: Some(state.cur_round),
        hero_sp_attributes: state.hero_sp_attributes.clone(),
        last_change_hero_uid: fight.last_change_hero_uid.or(state.last_change_hero_uid),
        ..Default::default()
    }
}

pub(crate) fn round_field_cards(cards: &[CardInfo]) -> Vec<CardInfo> {
    cards.iter().map(round_field_card).collect()
}

fn round_field_card(card: &CardInfo) -> CardInfo {
    CardInfo {
        energy: Some(0),
        ..card.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_projection_keeps_precast_cards_without_using_a_normal_slot() {
        let temp = CardInfo {
            skill_id: Some(2),
            temp_card: Some(true),
            ..Default::default()
        };
        let current = CardInfo {
            skill_id: Some(3),
            ..Default::default()
        };
        let cards = vec![current, temp];

        let round = next_round_shell(
            &Fight::default(),
            &RoundState::default(),
            true,
            &cards,
            &[],
            &[],
        );

        assert_eq!(
            round
                .before_cards1
                .iter()
                .filter_map(|card| card.skill_id)
                .collect::<Vec<_>>(),
            vec![3, 2]
        );
        assert!(round.team_a_cards1.is_empty());
        assert_eq!(round_field_cards(&cards).len(), 2);
    }

    #[test]
    fn finished_round_has_no_next_round_card_snapshots() {
        let cards = vec![CardInfo {
            skill_id: Some(1),
            ..Default::default()
        }];
        let state = RoundState {
            is_finish: true,
            before_cards2: cards.clone(),
            team_a_cards2: cards.clone(),
            ..Default::default()
        };

        let round = next_round_shell(&Fight::default(), &state, false, &cards, &cards, &[]);

        assert!(round.before_cards1.is_empty());
        assert!(round.team_a_cards1.is_empty());
        assert!(round.before_cards2.is_empty());
        assert!(round.team_a_cards2.is_empty());
    }

    #[test]
    fn late_terminal_round_keeps_already_prepared_card_snapshots() {
        let cards = vec![CardInfo {
            skill_id: Some(1),
            ..Default::default()
        }];
        let state = RoundState {
            is_finish: true,
            before_cards2: cards.clone(),
            team_a_cards2: cards.clone(),
            ..Default::default()
        };

        let round = next_round_shell(&Fight::default(), &state, true, &cards, &cards, &[]);

        assert_eq!(round.before_cards1, round_field_cards(&cards));
        assert_eq!(round.team_a_cards1, round_field_cards(&cards));
        assert_eq!(round.before_cards2, cards);
        assert_eq!(round.team_a_cards2, cards);
    }
}
