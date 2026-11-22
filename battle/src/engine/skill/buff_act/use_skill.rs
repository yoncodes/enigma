use crate::engine::skill::{
    action::SkillRequest,
    buff_act::registry::{self, BuffActKind},
    subscriber::BuffActSubscriber,
};

pub fn linked(subscriber: &BuffActSubscriber) -> Option<SkillRequest> {
    linked_for(
        subscriber.owner_uid,
        subscriber.key.definition.opcode,
        &subscriber.act_type,
        &subscriber.args,
    )
}

pub fn linked_for(
    owner_uid: i64,
    opcode: i32,
    type_name: &str,
    args: &[i32],
) -> Option<SkillRequest> {
    let skill_id = match registry::kind(opcode, type_name)? {
        BuffActKind::UseSkillToEnemy
        | BuffActKind::ConsumeBuffContinueChannel
        | BuffActKind::ConsumeBuffAddBuffContinueChannel => args.first(),
        BuffActKind::MonitorContinueChannel => args.get(1),
        _ => None,
    }
    .copied()
    .filter(|id| *id > 0)?;
    Some(SkillRequest {
        source_uid: owner_uid,
        skill_id,
    })
}

#[cfg(test)]
mod tests;
