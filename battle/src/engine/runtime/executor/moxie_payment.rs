use crate::engine::{
    event::{bus::EventBus, kind::EventKind, payload::BattleEvent},
    manager::{
        BattleManagers,
        ex_point::{ExPointChanges, ExPointCommand, ExPointCommandError},
    },
};

pub(super) fn pay(
    managers: &mut BattleManagers,
    events: &mut EventBus,
    payer: i64,
    skill_id: i32,
    command: ExPointCommand,
) -> Result<ExPointChanges, ExPointCommandError> {
    let (changes, paid) = managers.pay_skill_moxie(payer, skill_id, command)?;
    for event in changes.events() {
        events.push(event);
    }
    if let Some(paid) = paid
        && managers.buff.observes(EventKind::UltimateMoxiePaid)
    {
        events.push(BattleEvent::UltimateMoxiePaid(paid));
    }
    Ok(changes)
}
