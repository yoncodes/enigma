use super::{ExPointChanges, ExPointCommand, ExPointCommandError, ExPointKind, ExPointManager};
use crate::engine::manager::BattleManagers;

impl BattleManagers {
    pub(crate) fn pay_skill_moxie(
        &mut self,
        payer: i64,
        skill_id: i32,
        command: ExPointCommand,
    ) -> Result<ExPointChanges, ExPointCommandError> {
        let changes = self.execute_ex_point(command)?;
        if let ExPointChanges::Value { change, .. } = &changes
            && change.kind == ExPointKind::Common
            && change.target_uid == payer
            && change.applied_delta < 0
            && self.catalog().skill_is_big(skill_id)
            && let Some(team) = self.entity.team_type(payer)
        {
            self.ex_point
                .record_ultimate_spend(team, change.applied_delta.saturating_neg());
        }
        Ok(changes)
    }
}

impl ExPointManager {
    fn record_ultimate_spend(&mut self, team: i32, amount: i32) {
        let spent = self.round_ultimate_spend.entry(team).or_default();
        *spent = spent.saturating_add(amount);
    }

    pub(crate) fn round_ultimate_spend(&self, team: i32) -> i32 {
        self.round_ultimate_spend
            .get(&team)
            .copied()
            .unwrap_or_default()
    }

    pub(in crate::engine::manager) fn clear_round_ultimate_spend(&mut self) {
        self.round_ultimate_spend.clear();
    }
}
