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
    let mut actions = Vec::new();

    for trigger in db
        .trigger
        .iter()
        .filter(|trigger| trigger.battle_id == battle_id && trigger.trigger_type == "WaveStart")
    {
        let configured_wave =
            trigger
                .param2
                .parse::<i32>()
                .map_err(|_| BattleTriggerError::InvalidWave {
                    trigger_id: trigger.id,
                    value: trigger.param2.clone(),
                })?;
        if configured_wave != wave {
            continue;
        }

        for value in trigger
            .action_list
            .split(['#', '|', ','])
            .filter(|value| !value.is_empty())
        {
            let action_id =
                value
                    .parse::<i32>()
                    .map_err(|_| BattleTriggerError::InvalidActionId {
                        trigger_id: trigger.id,
                        value: value.to_owned(),
                    })?;
            let action =
                db.trigger_action
                    .get(action_id)
                    .ok_or(BattleTriggerError::MissingAction {
                        trigger_id: trigger.id,
                        action_id,
                    })?;
            let kind = match action.action_type.as_str() {
                "Prompt" => {
                    let prompt_id = action.param1.parse::<i32>().map_err(|_| {
                        BattleTriggerError::InvalidPromptId {
                            action_id,
                            value: action.param1.clone(),
                        }
                    })?;
                    if db.fight_prompt.get(prompt_id).is_none() {
                        return Err(BattleTriggerError::MissingPrompt {
                            action_id,
                            prompt_id,
                        });
                    }
                    TriggerActionKind::Prompt
                }
                unsupported => {
                    tracing::warn!(
                        battle_id,
                        wave,
                        trigger_id = trigger.id,
                        action_id,
                        action_type = unsupported,
                        "unsupported battle trigger action"
                    );
                    continue;
                }
            };
            actions.push(WaveStartAction {
                trigger_id: trigger.id,
                action_id,
                kind,
            });
        }
    }

    Ok(actions)
}

#[cfg(test)]
mod tests;
