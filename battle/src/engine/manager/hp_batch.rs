use std::collections::{HashMap, HashSet};

use super::{
    BattleManagers, HpCommit, HpExecution,
    hp::{HpChanges, HpCommand, HpCommandError, HpManager, LinkedHit},
};

enum BatchPart {
    Direct(Vec<HpCommand>),
    Linked(LinkedHit),
}

impl BatchPart {
    fn commands(&self) -> &[HpCommand] {
        match self {
            Self::Direct(commands) => commands,
            Self::Linked(hit) => &hit.commands,
        }
    }

    fn eligible(&self, hp: &HpManager) -> bool {
        match self {
            Self::Direct(_) => true,
            Self::Linked(hit) => hp.current(hit.target_uid) > 0,
        }
    }
}

impl BattleManagers {
    pub(crate) fn execute_hp_batch(
        &mut self,
        commands: Vec<HpCommand>,
    ) -> Result<Vec<HpChanges>, HpCommandError> {
        self.commit_hp_batch(vec![BatchPart::Direct(commands)])
            .map(|commits| commits.into_iter().map(|commit| commit.changes).collect())
    }

    pub(crate) fn execute_rule_hp_batch(
        &mut self,
        commands: Vec<HpCommand>,
    ) -> Result<Vec<HpExecution>, HpCommandError> {
        self.execute_hp_parts(vec![BatchPart::Direct(commands)])
    }

    pub(crate) fn execute_linked_damage(
        &mut self,
        hits: Vec<LinkedHit>,
    ) -> Result<Vec<HpExecution>, HpCommandError> {
        self.execute_hp_parts(hits.into_iter().map(BatchPart::Linked).collect())
    }

    fn execute_hp_parts(
        &mut self,
        parts: Vec<BatchPart>,
    ) -> Result<Vec<HpExecution>, HpCommandError> {
        let commits = self.commit_hp_batch(parts)?;
        Ok(commits
            .into_iter()
            .map(|commit| {
                let indicator = self.record_damage_indicator(
                    commit.changes.target_uid,
                    commit.changes.applied_damage(),
                );
                HpExecution {
                    changes: commit.changes,
                    resolved_heal: commit.resolved_heal,
                    before_hp_markers: commit.before_hp_markers,
                    after_hp_markers: commit.after_hp_markers,
                    shield_counter_credits: commit.shield_counter_credits,
                    indicator,
                }
            })
            .collect())
    }

    fn commit_hp_batch(
        &mut self,
        mut parts: Vec<BatchPart>,
    ) -> Result<Vec<HpCommit>, HpCommandError> {
        parts.retain(|part| part.eligible(&self.hp));
        let mut targets_by_team = HashMap::<i32, HashSet<i64>>::new();
        for command in parts.iter().flat_map(BatchPart::commands) {
            if let HpCommand::Damage(damage) | HpCommand::SettleAccumulatedDamage(damage) = command
                && damage.amount > 0
                && let Some(team) = self.entity.team_type(damage.target_uid)
            {
                targets_by_team
                    .entry(team)
                    .or_default()
                    .insert(damage.target_uid);
            }
        }
        let mut staged = self.clone();
        let mut shared_values = HashMap::new();
        let mut share_spent = HashMap::new();
        let mut plans = Vec::new();
        for part in parts {
            if !part.eligible(&staged.hp) {
                continue;
            }
            for &command in part.commands() {
                if matches!(part, BatchPart::Linked(_)) && staged.hp.current(target(command)) <= 0 {
                    continue;
                }
                let target_count = match command {
                    HpCommand::Damage(damage) | HpCommand::SettleAccumulatedDamage(damage) => self
                        .entity
                        .team_type(damage.target_uid)
                        .and_then(|team| targets_by_team.get(&team))
                        .map_or(1, HashSet::len),
                    _ => 1,
                };
                let mut plan = staged.plan_hp(
                    command,
                    target_count,
                    &staged.hp,
                    &mut shared_values,
                    &mut share_spent,
                )?;
                staged.prepare_damage_state(&mut plan)?;
                staged.commit_hp_preview(&plan);
                plans.push(plan);
            }
        }
        Ok(plans.into_iter().map(|plan| self.commit_hp(plan)).collect())
    }
}

fn target(command: HpCommand) -> i64 {
    match command {
        HpCommand::Damage(value) | HpCommand::SettleAccumulatedDamage(value) => value.target_uid,
        HpCommand::Lose(value) => value.target_uid,
        HpCommand::Kill(value) => value.target_uid,
        HpCommand::Heal(value) => value.target_uid,
        HpCommand::AdjustMax(value)
        | HpCommand::RestoreMax(value)
        | HpCommand::RemoveReplacedMax(value)
        | HpCommand::AdjustCapacity(value) => value.target_uid,
        HpCommand::SetCurrent(value) => value.target_uid,
        HpCommand::GrantShield(value) => value.target_uid,
    }
}

#[cfg(test)]
mod tests;
