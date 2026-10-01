use sonettobuf::{CardInfo, FightRound, FightStep, effect_type_enum::EffectType};

use super::BattleRuntime;

impl BattleRuntime {
    /// Preview diagnostics only: copies the captured HP, ex points, and next-round hand into
    /// the managers so later rounds can replay past an earlier divergence. Never call this
    /// from the game server; resynced rounds are not parity evidence.
    pub fn resync_observed_state(&mut self, captured: &FightRound) -> Result<Vec<String>, String> {
        if self.round_state.is_finish != captured.is_finish.unwrap_or_default() {
            return Err(format!(
                "finish state differs: generated={} captured={}",
                self.round_state.is_finish,
                captured.is_finish.unwrap_or_default()
            ));
        }
        let mut changes = Vec::new();
        for info in &captured.ex_point_info {
            let Some(uid) = info.uid else {
                continue;
            };
            if let Some(hp) = info.current_hp {
                let generated = self.managers.hp.current(uid);
                if (generated > 0) != (hp > 0) {
                    return Err(format!(
                        "life state differs uid={uid}: generated hp={generated} captured hp={hp}"
                    ));
                }
                if generated != hp {
                    self.managers.hp.resync_current(uid, hp);
                    let resynced = self.managers.hp.current(uid);
                    changes.push(format!("hp uid={uid} {generated}->{resynced}"));
                }
            }
            if let Some(ex_point) = info.ex_point {
                let generated = self.managers.ex_point.get(uid);
                if generated != ex_point {
                    self.managers.ex_point.resync_current(uid, ex_point);
                    let resynced = self.managers.ex_point.get(uid);
                    changes.push(format!("exPoint uid={uid} {generated}->{resynced}"));
                }
            }
        }
        if let Some(hand) = captured_next_round_hand(captured)
            && !same_cards(self.managers.card.hand(), &hand)
        {
            changes.push(format!(
                "hand {:?}->{:?}",
                skill_ids(self.managers.card.hand()),
                skill_ids(&hand)
            ));
            self.managers.card.resync_hand(hand);
        }
        self.managers.sync_entities(&mut self.fight);
        Ok(changes)
    }
}

// The last card push published while preparing the next round is the hand the client holds.
fn captured_next_round_hand(round: &FightRound) -> Option<Vec<CardInfo>> {
    fn visit(step: &FightStep, hand: &mut Option<Vec<CardInfo>>) {
        for effect in &step.act_effect {
            if effect.effect_type == Some(EffectType::Cardspush as i32) {
                *hand = Some(effect.card_info_list.clone());
            }
            if let Some(nested) = effect.fight_step.as_ref() {
                visit(nested, hand);
            }
        }
    }
    let mut hand = None;
    for step in &round.next_round_begin_step {
        visit(step, &mut hand);
    }
    hand
}

fn same_cards(left: &[CardInfo], right: &[CardInfo]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| left.uid == right.uid && left.skill_id == right.skill_id)
}

fn skill_ids(cards: &[CardInfo]) -> Vec<i32> {
    cards.iter().filter_map(|card| card.skill_id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonettobuf::{Fight, FightEntityInfo, FightExPointInfo, FightStep, FightTeam};

    fn runtime() -> BattleRuntime {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    ex_point: Some(1),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        BattleRuntime {
            managers: crate::engine::manager::BattleManagers::seeded(&fight),
            fight,
            ..Default::default()
        }
    }

    fn card(uid: i64, skill_id: i32) -> CardInfo {
        CardInfo {
            uid: Some(uid),
            skill_id: Some(skill_id),
            ..Default::default()
        }
    }

    #[test]
    fn copies_captured_hp_ex_point_and_last_published_hand() {
        let mut runtime = runtime();
        let captured = FightRound {
            ex_point_info: vec![FightExPointInfo {
                uid: Some(10),
                ex_point: Some(4),
                current_hp: Some(60),
                ..Default::default()
            }],
            next_round_begin_step: vec![FightStep {
                act_effect: vec![
                    sonettobuf::ActEffect {
                        effect_type: Some(EffectType::Cardspush as i32),
                        card_info_list: vec![card(10, 1)],
                        ..Default::default()
                    },
                    sonettobuf::ActEffect {
                        card_info_list: vec![card(10, 9)],
                        ..Default::default()
                    },
                    sonettobuf::ActEffect {
                        fight_step: Some(FightStep {
                            act_effect: vec![sonettobuf::ActEffect {
                                effect_type: Some(EffectType::Cardspush as i32),
                                card_info_list: vec![card(10, 1), card(10, 2)],
                                ..Default::default()
                            }],
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let changes = runtime.resync_observed_state(&captured).unwrap();

        assert_eq!(changes.len(), 3);
        assert_eq!(runtime.managers.hp.current(10), 60);
        assert_eq!(runtime.managers.ex_point.get(10), 4);
        assert_eq!(skill_ids(runtime.card_hand()), vec![1, 2]);
    }

    #[test]
    fn refuses_to_revive_or_continue_a_finished_battle() {
        let mut runtime = runtime();
        let dead = FightRound {
            ex_point_info: vec![FightExPointInfo {
                uid: Some(10),
                current_hp: Some(0),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(runtime.resync_observed_state(&dead).is_err());
        assert_eq!(runtime.managers.hp.current(10), 100);

        runtime.round_state.is_finish = true;
        assert!(
            runtime
                .resync_observed_state(&FightRound::default())
                .is_err()
        );
    }
}
