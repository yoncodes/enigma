use super::{HpCommand, HpHealKind};
use crate::engine::{
    event::payload::{BattleEvent, HealResolvedEvent},
    manager::HpExecution,
};

pub(crate) fn resolved_amount(command: HpCommand) -> Option<i32> {
    let HpCommand::Heal(heal) = command else {
        return None;
    };
    matches!(heal.kind, HpHealKind::Normal | HpHealKind::Critical).then_some(heal.amount)
}

impl HpExecution {
    pub(crate) fn events(&self) -> Vec<BattleEvent> {
        let mut events = self.changes.events();
        if let Some(amount) = self.resolved_heal {
            events.push(BattleEvent::HealResolved(HealResolvedEvent {
                source_uid: self.changes.source_uid,
                target_uid: self.changes.target_uid,
                amount,
                cast: None,
            }));
        }
        events
    }
}
