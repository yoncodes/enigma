use sonettobuf::{BeginRoundOper, CardInfo};

use crate::engine::manager::card::CardOpType;

#[derive(Debug, Clone, PartialEq)]
pub enum PlayerQueuedAction {
    Card(Box<CardInfo>),
    Conduit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerActionQueue {
    pub team: i32,
    pub emitter_uid: i64,
    pub actions: Vec<PlayerQueuedAction>,
}

impl PlayerActionQueue {
    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    pub fn cards(&self) -> Vec<CardInfo> {
        self.actions
            .iter()
            .filter_map(|action| match action {
                PlayerQueuedAction::Card(card) => Some(card.as_ref().clone()),
                PlayerQueuedAction::Conduit => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConduitSelection {
    pub source_uid: i64,
    pub team: i32,
    pub group: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConduitSelections {
    pub selections: Vec<ConduitSelection>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoundCommand {
    MoveCard {
        from_index: usize,
        to_index: usize,
    },
    UseUniversal {
        universal_index: usize,
        target_index: usize,
    },
    DissolveCard {
        card_index: usize,
    },
    UseAssistBoss {
        skill_id: i32,
        target_uid: Option<i64>,
    },
    PlayCard {
        card_index: usize,
        target_uid: Option<i64>,
        chosen_skill_id: Option<i32>,
        recorded_skill: Option<crate::engine::skill::action::SkillRequest>,
    },
}

impl RoundCommand {
    pub fn from_oper(oper: &BeginRoundOper) -> Option<Self> {
        let oper_type = CardOpType::from(oper.oper_type.unwrap_or_default());
        let target_uid = oper.to_id.filter(|uid| *uid != 0);
        if oper_type == CardOpType::SimulateDissolveCard {
            return Some(Self::DissolveCard {
                card_index: oper.param1.unwrap_or(1).saturating_sub(1) as usize,
            });
        }

        if oper_type == CardOpType::MoveCard && target_uid.is_none() {
            return oper.param2.map(|to_index| Self::MoveCard {
                from_index: oper.param1.unwrap_or(1).saturating_sub(1) as usize,
                to_index: to_index.saturating_sub(1) as usize,
            });
        }
        if oper_type == CardOpType::MoveUniversal {
            return Some(Self::UseUniversal {
                universal_index: oper.param1?.saturating_sub(1) as usize,
                target_index: oper.param2?.saturating_sub(1) as usize,
            });
        }
        if oper_type == CardOpType::AssistBoss {
            return oper
                .param1
                .filter(|skill_id| *skill_id > 0)
                .map(|skill_id| Self::UseAssistBoss {
                    skill_id,
                    target_uid,
                });
        }

        let plays_card = oper_type == CardOpType::PlayCard;
        let moves_to_target = oper_type == CardOpType::MoveCard && target_uid.is_some();
        if !plays_card && !moves_to_target {
            return None;
        }

        Some(Self::PlayCard {
            card_index: oper.param1.unwrap_or(1).saturating_sub(1) as usize,
            target_uid,
            chosen_skill_id: oper.param3.filter(|skill_id| *skill_id != 0),
            recorded_skill: parse_recorded_skill(oper.card_param1.as_deref()),
        })
    }
}

fn parse_recorded_skill(raw: Option<&str>) -> Option<crate::engine::skill::action::SkillRequest> {
    let mut values = raw?.split('#');
    let skill_id = values.next()?.parse().ok()?;
    let source_uid = values.next()?.parse().ok()?;
    (skill_id > 0 && source_uid != 0 && values.next().is_none()).then_some(
        crate::engine::skill::action::SkillRequest {
            source_uid,
            skill_id,
        },
    )
}

pub fn commands_from_opers(opers: &[BeginRoundOper]) -> Vec<RoundCommand> {
    opers.iter().filter_map(RoundCommand::from_oper).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_play_card_commands_from_begin_round_opers() {
        let opers = vec![
            BeginRoundOper {
                oper_type: Some(1),
                param1: Some(1),
                param2: Some(2),
                to_id: Some(0),
                ..Default::default()
            },
            BeginRoundOper {
                oper_type: Some(2),
                param1: Some(3),
                to_id: Some(-1),
                param3: Some(99),
                ..Default::default()
            },
            BeginRoundOper {
                oper_type: Some(3),
                param1: Some(2),
                param2: Some(3),
                ..Default::default()
            },
            BeginRoundOper {
                oper_type: Some(-99),
                param1: Some(1),
                ..Default::default()
            },
            BeginRoundOper {
                oper_type: Some(4),
                param1: Some(31_300_001),
                to_id: Some(-1),
                ..Default::default()
            },
        ];

        assert_eq!(
            commands_from_opers(&opers),
            vec![
                RoundCommand::MoveCard {
                    from_index: 0,
                    to_index: 1,
                },
                RoundCommand::PlayCard {
                    card_index: 2,
                    target_uid: Some(-1),
                    chosen_skill_id: Some(99),
                    recorded_skill: None,
                },
                RoundCommand::UseUniversal {
                    universal_index: 1,
                    target_index: 2,
                },
                RoundCommand::DissolveCard { card_index: 0 },
                RoundCommand::UseAssistBoss {
                    skill_id: 31_300_001,
                    target_uid: Some(-1),
                },
            ]
        );
    }
}
