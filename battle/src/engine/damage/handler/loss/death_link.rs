use super::{AttrId, BattleEvent, BehaviorOpContext, Loss, ParsedBehavior};

pub(super) fn amount(context: &BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<i32> {
    let Some(BattleEvent::EntityDied(death)) = context.event else {
        return None;
    };
    if death.target_uid != context.source_uid
        || !matches!(behavior.args.as_slice(), [0, attr, _] if AttrId::from_raw(*attr) == Some(AttrId::Hp))
    {
        return None;
    }

    let loss = Loss::read_from(context, behavior, death.target_uid)?;
    Some(if loss.current > 0 && loss.floor == 0 {
        loss.requested
    } else {
        loss.limited()
    })
}
