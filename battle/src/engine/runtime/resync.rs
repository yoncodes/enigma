use std::collections::BTreeSet;

use sonettobuf::{ActEffect, CardInfo, FightRound, FightStep, effect_type_enum::EffectType};

use super::BattleRuntime;

impl BattleRuntime {
    /// Preview diagnostics only: copies captured HP, ex points, power resources, opponent
    /// self-owned buffs, and the next-round hand into managers so later rounds can replay past an
    /// earlier divergence.
    /// Never call this from the game server; resynced rounds are not parity evidence.
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
            for power in &info.power_infos {
                let (Some(power_id), Some(current), Some(max)) =
                    (power.power_id, power.num, power.max)
                else {
                    return Err(format!("captured power state for uid={uid} is incomplete"));
                };
                let (before, after) = self
                    .managers
                    .eureka
                    .resync_observed(uid, power_id, current, max)
                    .ok_or_else(|| {
                        format!(
                            "captured power state is invalid uid={uid} powerId={power_id} num={current} max={max}"
                        )
                    })?;
                if before != after {
                    changes.push(format!(
                        "power uid={uid} id={power_id} {}/{}->{}/{}",
                        before.current, before.max, after.current, after.max
                    ));
                }
            }
        }
        let opponent_uids = opponent_uids(&self.fight);
        let mut changed_buffs = BTreeSet::new();
        for effect in captured_buff_effects(captured) {
            let Some(kind) = effect.effect_type else {
                continue;
            };
            if ![
                EffectType::Buffadd as i32,
                EffectType::Buffupdate as i32,
                EffectType::Buffdel as i32,
            ]
            .contains(&kind)
            {
                continue;
            }
            let target_uid = effect
                .target_id
                .ok_or_else(|| format!("captured buff effect type={kind} has no target uid"))?;
            if !opponent_uids.contains(&target_uid) {
                continue;
            }
            let buff = effect
                .buff
                .clone()
                .ok_or_else(|| format!("captured buff effect type={kind} has no buff payload"))?;
            // Player-sourced debuffs on an opponent are coupled to player-owned mechanic state.
            // Replaying only their wire half would make the diagnostic runtime inconsistent.
            if buff.from_uid != Some(target_uid) {
                continue;
            }
            let buff_uid = buff.uid.filter(|uid| *uid > 0).ok_or_else(|| {
                format!("captured buff effect type={kind} on target {target_uid} has no valid uid")
            })?;
            let changed = match kind {
                kind if kind == EffectType::Buffadd as i32
                    || kind == EffectType::Buffupdate as i32 =>
                {
                    self.managers
                        .buff
                        .resync_observed_present(target_uid, buff)?
                }
                kind if kind == EffectType::Buffdel as i32 => self
                    .managers
                    .buff
                    .resync_observed_removed(target_uid, buff_uid),
                _ => unreachable!("buff effect kinds were filtered above"),
            };
            if changed {
                changed_buffs.insert((target_uid, buff_uid));
            }
        }
        self.managers.buff.finish_observed_resync();
        changes.extend(
            changed_buffs
                .into_iter()
                .map(|(target_uid, buff_uid)| format!("buff uid={buff_uid} target={target_uid}")),
        );
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

fn opponent_uids(fight: &sonettobuf::Fight) -> BTreeSet<i64> {
    let mut uids = BTreeSet::new();
    if let Some(team) = fight.defender.as_ref() {
        uids.extend(
            team.entitys
                .iter()
                .chain(&team.sub_entitys)
                .filter_map(|entity| entity.uid),
        );
        uids.extend(team.assist_boss.iter().filter_map(|entity| entity.uid));
    }
    uids
}

fn captured_buff_effects(round: &FightRound) -> Vec<&ActEffect> {
    fn visit<'a>(step: &'a FightStep, effects: &mut Vec<&'a ActEffect>) {
        for effect in &step.act_effect {
            effects.push(effect);
            if let Some(nested) = effect.fight_step.as_ref() {
                visit(nested, effects);
            }
        }
    }

    let mut effects = Vec::new();
    for step in round.fight_step.iter().chain(&round.next_round_begin_step) {
        visit(step, &mut effects);
    }
    effects
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
    use sonettobuf::{
        BuffInfo, Fight, FightEntityInfo, FightExPointInfo, FightStep, FightTeam, PowerInfo,
    };

    fn runtime() -> BattleRuntime {
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    ex_point: Some(1),
                    power_infos: vec![PowerInfo {
                        power_id: Some(1),
                        num: Some(5),
                        max: Some(8),
                    }],
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    attr: Some(sonettobuf::HeroAttribute {
                        hp: Some(100),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(100001),
                        buff_id: Some(109320109),
                        layer: Some(1),
                        ..Default::default()
                    }],
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
                power_infos: vec![PowerInfo {
                    power_id: Some(1),
                    num: Some(0),
                    max: Some(8),
                }],
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

        assert_eq!(changes.len(), 4);
        assert_eq!(runtime.managers.hp.current(10), 60);
        assert_eq!(runtime.managers.ex_point.get(10), 4);
        assert_eq!(runtime.managers.eureka.get(10, 1).current, 0);
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

    #[test]
    fn folds_captured_buff_changes_into_manager_owned_state() {
        let mut runtime = runtime();
        let observed = |effect_type, source_uid, target_uid, uid, layer| ActEffect {
            target_id: Some(target_uid),
            effect_type: Some(effect_type as i32),
            buff: Some(BuffInfo {
                uid: Some(uid),
                buff_id: Some(109320109),
                from_uid: Some(source_uid),
                layer: Some(layer),
                ..Default::default()
            }),
            ..Default::default()
        };
        let captured = FightRound {
            fight_step: vec![FightStep {
                act_effect: vec![
                    observed(EffectType::Buffdel, -1, -1, 100001, 0),
                    observed(EffectType::Buffadd, -1, -1, 100003, 1),
                    observed(EffectType::Buffadd, 10, 10, 4, 1),
                    observed(EffectType::Buffadd, 10, -1, 100005, 1),
                    ActEffect {
                        fight_step: Some(FightStep {
                            act_effect: vec![observed(EffectType::Buffupdate, -1, -1, 100003, 2)],
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        runtime.resync_observed_state(&captured).unwrap();

        assert!(runtime.managers.buff.snapshot(-1, 100001).is_none());
        assert!(runtime.managers.buff.snapshot(10, 4).is_none());
        assert!(runtime.managers.buff.snapshot(-1, 100005).is_none());
        assert_eq!(
            runtime.managers.buff.snapshot(-1, 100003).unwrap().layer,
            Some(2)
        );
        assert_eq!(
            runtime.fight.defender.as_ref().unwrap().entitys[0]
                .buffs
                .iter()
                .find(|buff| buff.uid == Some(100003))
                .and_then(|buff| buff.layer),
            Some(2)
        );
    }
}
