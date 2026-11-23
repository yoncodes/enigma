use super::*;
use crate::engine::event::payload::BattleEvent;

fn separate(behavior: &ParsedBehavior) -> Option<bool> {
    if behavior.raw_arity != behavior.args.len() {
        return None;
    }
    match behavior.args.as_slice() {
        [buff_id] if *buff_id > 0 => Some(false),
        [buff_id, 1] if *buff_id > 0 => Some(true),
        _ => None,
    }
}

pub(in crate::engine::skill::behavior) fn supports(behavior: &ParsedBehavior) -> bool {
    separate(behavior).is_some()
}

pub(super) fn rule_ops(
    context: &BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
) -> Option<Vec<RuleOp>> {
    let separate = separate(behavior)?;
    let buff_id = behavior.arg(0)?;
    let BattleEvent::SkillAction(action) = context.event? else {
        return Some(Vec::new());
    };
    let count = action
        .buff_additions
        .iter()
        .find(|addition| addition.buff_id == buff_id)
        .map(|addition| {
            if separate {
                addition.admitted
            } else {
                addition.stored
            }
        })
        .unwrap_or_default();
    if count <= 0 {
        return Some(Vec::new());
    }
    let occurrences = if separate { 1 } else { count as u32 };
    let command = grant_command(
        context.source_uid,
        context.target_uid,
        occurrences,
        context.event,
        behavior,
    )?;
    let op = RuleOp::Command(BattleCommand::Buff(command));
    Some(vec![op; if separate { count as usize } else { 1 }])
}
