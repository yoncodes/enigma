use super::{BattleEvent, SkillOpTrigger};
use crate::engine::event::payload::HealCast;

pub(super) fn attach(
    events: &mut [BattleEvent],
    event_groups: &mut [Vec<BattleEvent>],
    current_skill_id: Option<i32>,
    trigger: &SkillOpTrigger,
) {
    let cast = current_skill_id.map(HealCast::Direct).or(match trigger {
        SkillOpTrigger::Event(BattleEvent::AllyAction(action)) => {
            Some(HealCast::Reaction(action.skill_id))
        }
        _ => None,
    });
    if let Some(cast) = cast {
        for event in events.iter_mut().chain(event_groups.iter_mut().flatten()) {
            if let BattleEvent::HealResolved(heal) = event {
                heal.cast = Some(cast);
            }
        }
    }
}
