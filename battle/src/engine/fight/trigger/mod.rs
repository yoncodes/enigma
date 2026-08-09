use crate::engine::skill::rule::output::RuleOp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerActionKind {
    Prompt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveStartAction {
    pub trigger_id: i32,
    pub action_id: i32,
    pub kind: TriggerActionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BattleTriggerError {
    InvalidWave { trigger_id: i32, value: String },
    InvalidActionId { trigger_id: i32, value: String },
    MissingAction { trigger_id: i32, action_id: i32 },
    InvalidPromptId { action_id: i32, value: String },
    MissingPrompt { action_id: i32, prompt_id: i32 },
}

impl WaveStartAction {
    pub fn rule_op(self) -> RuleOp {
        RuleOp::EffectMarker {
            target_uid: super::rules::DEFENDER_SIDE_UID,
            effect_type: sonettobuf::effect_type_enum::EffectType::Trigger as i32,
            effect_num: self.action_id,
            config_effect: 0,
            reserve_id: None,
            reserve_str: None,
        }
    }
}

pub fn wave_start_actions(
    db: &config::configs::GameDB,
    battle_id: i32,
    wave: i32,
) -> Result<Vec<WaveStartAction>, BattleTriggerError> {
    crate::catalog::configured_wave_start_actions(db, battle_id, wave)
}

#[cfg(test)]
mod tests;
