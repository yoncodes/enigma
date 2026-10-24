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
        let player_uids = player_uids(&self.fight);
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
            let buff = effect
                .buff
                .clone()
                .ok_or_else(|| format!("captured buff effect type={kind} has no buff payload"))?;
            let source_uid = buff.from_uid.unwrap_or_default();
            let safe_observed_owner = (opponent_uids.contains(&target_uid)
                && source_uid == target_uid)
                || (player_uids.contains(&target_uid) && opponent_uids.contains(&source_uid));
            // Player-owned effects stay coupled to their mechanic state; opponent-owned effects
            // can be copied onto either side without inventing player state.
            if !safe_observed_owner {
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

fn player_uids(fight: &sonettobuf::Fight) -> BTreeSet<i64> {
    let mut uids = BTreeSet::new();
    if let Some(team) = fight.attacker.as_ref() {
        uids.extend(
            team.entitys
                .iter()
                .chain(&team.sub_entitys)
                .filter_map(|entity| entity.uid),
        );
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
mod tests;
