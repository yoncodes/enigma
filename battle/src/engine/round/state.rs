use sonettobuf::{CardInfo, Fight, FightHeroSpAttributeInfo, FightRound};

use crate::engine::{
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
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
    let buff_bonus = super::modifier::active_buff_action_bonus(managers, 1);
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
        round_before_cards1(current_hand)
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
            state
                .before_cards2
                .iter()
                .filter(|card| !card.temp_card.unwrap_or_default())
                .cloned()
                .collect()
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

fn round_before_cards1(cards: &[CardInfo]) -> Vec<CardInfo> {
    cards
        .iter()
        .filter(|card| !is_owner_bound_generic_temp(card))
        .map(round_field_card)
        .collect()
}

fn is_owner_bound_generic_temp(card: &CardInfo) -> bool {
    card.temp_card.unwrap_or_default()
        && card.uid.unwrap_or_default() != 0
        && card.card_type.unwrap_or_default() == 0
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
    fn round_projection_uses_field_specific_temporary_card_rules() {
        let cards = vec![
            CardInfo {
                uid: Some(1),
                skill_id: Some(1),
                energy: Some(0),
                ..Default::default()
            },
            CardInfo {
                uid: Some(10),
                skill_id: Some(2),
                temp_card: Some(true),
                card_type: Some(0),
                hero_id: Some(3149),
                energy: Some(0),
                ..Default::default()
            },
            CardInfo {
                uid: Some(10),
                skill_id: Some(3),
                temp_card: Some(true),
                card_type: Some(sonettobuf::card_info::CardType::Skill3 as i32),
                hero_id: Some(3149),
                energy: Some(0),
                ..Default::default()
            },
            CardInfo {
                uid: Some(0),
                skill_id: Some(4),
                temp_card: Some(true),
                energy: Some(0),
                ..Default::default()
            },
        ];
        let state = RoundState {
            before_cards2: cards.clone(),
            team_a_cards2: cards.clone(),
            ..Default::default()
        };

        let round = next_round_shell(&Fight::default(), &state, true, &cards, &cards, &[]);
        let skills = |cards: &[CardInfo]| {
            cards
                .iter()
                .filter_map(|card| card.skill_id)
                .collect::<Vec<_>>()
        };

        assert_eq!(skills(&round.before_cards1), vec![1, 3, 4]);
        assert_eq!(skills(&round.before_cards2), vec![1]);
        assert_eq!(round.team_a_cards1, round_field_cards(&cards));
        assert_eq!(round.team_a_cards2, cards);
        assert_eq!(state.before_cards2, cards);
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
