use crate::engine::skill::{
    behavior::registry::{self, ADD_BUFF_KEY},
    buff_act::registry::{BuffActKind, resolve_feature},
    condition::{ParsedCondition, registry::BUFF_GATE_HAS_ID_KEY},
    effect::ParsedBehavior,
    rule::{SetupStage, route::ConditionSetup},
};

pub(crate) fn grant_setup(
    db: &config::GameDB,
    behavior: &ParsedBehavior,
    conditions: &[ParsedCondition],
) -> Option<ConditionSetup> {
    let gate = BUFF_GATE_HAS_ID_KEY;
    if registry::find(behavior)?.key != ADD_BUFF_KEY
        || !conditions.iter().any(|condition| {
            condition.opcode == gate.opcode && condition.type_name == gate.type_name
        })
    {
        return None;
    }
    let buff_id = behavior.arg(0)?;
    let buff = db.skill_buff.get(buff_id)?;
    buff.features
        .split('|')
        .filter_map(|feature| resolve_feature(Some(db), feature))
        .any(|feature| {
            feature.arguments_supported
                && feature.kind == Some(BuffActKind::BeatBack)
                && super::holder_consumed_buff(&feature.args) == Some(buff_id)
        })
        .then_some(ConditionSetup {
            key: gate,
            stage: SetupStage::RoundStart,
            priority: 1,
        })
}
