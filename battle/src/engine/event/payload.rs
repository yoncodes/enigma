use crate::engine::manager::card::CardChangeKind;
use crate::engine::manager::ex_point::ExPointKind;
use crate::engine::manager::field::FieldChangeKind;
use crate::engine::manager::gauge::{GaugeChangeKind, GaugeKey};
use crate::engine::{event::kind::EventKind, skill::rule::CommandOrigin};
use sonettobuf::CardInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffChangeEvent {
    pub source_uid: i64,
    pub target_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub before_amount: i32,
    pub after_amount: i32,
    pub act_id: i32,
    pub act_value: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffFeatureTriggeredEvent {
    pub owner_uid: i64,
    pub source_uid: i64,
    pub target_uid: i64,
    pub buff_uid: i64,
    pub buff_id: i32,
    pub act_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitEvent {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub skill_id: i32,
    pub amount: i32,
    pub damage_from: crate::engine::manager::hp::HurtDamageFromType,
    pub assassinate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntityDiedEvent {
    pub source_uid: i64,
    pub target_uid: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExPointChangeEvent {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub kind: ExPointKind,
    pub before: i32,
    pub requested_delta: i32,
    pub applied_delta: i32,
    pub after: i32,
    pub overflow: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EurekaChangeEvent {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub power_id: i32,
    pub before: i32,
    pub requested_delta: i32,
    pub applied_delta: i32,
    pub after: i32,
    pub overflow: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConduitActivatedEvent {
    pub source_uid: i64,
    pub team: i32,
    pub skill_id: i32,
    pub power_id: i32,
    pub spent: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GaugeChangeEvent {
    pub origin: CommandOrigin,
    pub key: GaugeKey,
    pub source_uid: i64,
    pub source_skill_id: i32,
    pub config_effect: i32,
    pub progress_raw_delta: i32,
    pub kind: GaugeChangeKind,
    pub before: i32,
    pub requested_delta: i32,
    pub applied_delta: i32,
    pub after: i32,
    pub overflow: i32,
    pub before_max: Option<i32>,
    pub after_max: Option<i32>,
    pub enabled_before: bool,
    pub enabled_after: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardChangeEvent {
    pub origin: CommandOrigin,
    pub kind: CardChangeKind,
    pub before_count: i32,
    pub after_count: i32,
    pub before_energy: i32,
    pub after_energy: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldChangeEvent {
    pub origin: CommandOrigin,
    pub team: i32,
    pub kind: FieldChangeKind,
    pub field_id: i32,
    pub before_level: i32,
    pub after_level: i32,
    pub before_progress: i32,
    pub after_progress: i32,
    pub overflow: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SummonChangeEvent {
    pub origin: CommandOrigin,
    pub owner_uid: i64,
    pub summoned_id: i32,
    pub before_count: i32,
    pub after_count: i32,
    pub before_level: i32,
    pub after_level: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellChangeEvent {
    pub kind: crate::engine::mechanic::shell::ShellChangeKind,
    pub source_uid: i64,
    pub target_uid: i64,
    pub stock_buff_id: i32,
    pub deployed_buff_id: i32,
    pub amount: i32,
    pub transaction_amount: i32,
    pub settles_transaction: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BattleEvent {
    EnterFight,
    EntityEntered {
        target_uid: i64,
    },
    EntityTransformed {
        target_uid: i64,
    },
    RoundStart,
    ActionQueueCommitted {
        team: i32,
        emitter_uid: i64,
        cards: Vec<CardInfo>,
    },
    PlayerActionsResolved {
        team: i32,
        emitter_uid: i64,
    },
    ImpromptuResolved {
        team: i32,
        emitter_uid: i64,
        critical_action_count: i32,
    },
    SkillEffectStarted(crate::engine::skill::action::SkillActionEvent),
    SkillAction(crate::engine::skill::action::SkillActionEvent),
    AllyAction(crate::engine::skill::action::ActionEvent),
    BuffAdded(BuffChangeEvent),
    BuffChanged(BuffChangeEvent),
    BuffRemoved(BuffChangeEvent),
    BuffsSettled(Vec<BuffChangeEvent>),
    BuffFeatureTriggered(BuffFeatureTriggeredEvent),
    HpLost {
        origin: CommandOrigin,
        source_uid: i64,
        skill_id: i32,
        target_uid: i64,
        amount: i32,
        buff_uid: Option<i64>,
    },
    HpHealed {
        origin: CommandOrigin,
        source_uid: i64,
        target_uid: i64,
        amount: i32,
    },
    ToughnessBroken {
        source_uid: i64,
        target_uid: i64,
        skill_id: i32,
    },
    Hit(HitEvent),
    EntityDied(EntityDiedEvent),
    BattleTerminalCommitted {
        outcome: crate::engine::round::outcome::BattleOutcome,
        winning_team: i32,
    },
    ExPointChanged(ExPointChangeEvent),
    ExPointOverflow(ExPointChangeEvent),
    EurekaChanged(EurekaChangeEvent),
    ConduitActivated(ConduitActivatedEvent),
    GaugeChanged(GaugeChangeEvent),
    CardChanged(CardChangeEvent),
    FieldChanged(FieldChangeEvent),
    SummonChanged(SummonChangeEvent),
    ShellChanged(ShellChangeEvent),
    BloodtitheChanged {
        team: i32,
        before: i32,
        delta: i32,
        after: i32,
    },
    Kind(EventKind),
}

impl BattleEvent {
    pub fn target_uid(&self) -> Option<i64> {
        match self {
            Self::EntityEntered { target_uid }
            | Self::EntityTransformed { target_uid }
            | Self::HpLost { target_uid, .. }
            | Self::HpHealed { target_uid, .. }
            | Self::ToughnessBroken { target_uid, .. } => Some(*target_uid),
            Self::SkillEffectStarted(action) | Self::SkillAction(action) => Some(action.target_uid),
            Self::AllyAction(action) => Some(action.target_uid),
            Self::BuffAdded(change) | Self::BuffChanged(change) | Self::BuffRemoved(change) => {
                Some(change.target_uid)
            }
            Self::BuffFeatureTriggered(trigger) => Some(trigger.target_uid),
            Self::Hit(hit) => Some(hit.target_uid),
            Self::EntityDied(death) => Some(death.target_uid),
            Self::ExPointChanged(change) | Self::ExPointOverflow(change) => Some(change.target_uid),
            Self::EurekaChanged(change) => Some(change.target_uid),
            Self::ConduitActivated(change) => Some(change.source_uid),
            Self::SummonChanged(change) => Some(change.owner_uid),
            Self::ShellChanged(change) => Some(change.target_uid),
            Self::EnterFight
            | Self::BattleTerminalCommitted { .. }
            | Self::RoundStart
            | Self::ActionQueueCommitted { .. }
            | Self::PlayerActionsResolved { .. }
            | Self::ImpromptuResolved { .. }
            | Self::GaugeChanged(_)
            | Self::CardChanged(_)
            | Self::FieldChanged(_)
            | Self::BuffsSettled(_)
            | Self::BloodtitheChanged { .. }
            | Self::Kind(_) => None,
        }
    }

    pub fn kind(&self) -> EventKind {
        match self {
            Self::EnterFight => EventKind::EnterFight,
            Self::EntityEntered { .. } => EventKind::EntityEntered,
            Self::EntityTransformed { .. } => EventKind::EntityTransformed,
            Self::RoundStart => EventKind::RoundStart,
            Self::ActionQueueCommitted { .. } => EventKind::ActionQueueCommitted,
            Self::PlayerActionsResolved { .. } => EventKind::PlayerActionsResolved,
            Self::ImpromptuResolved { .. } => EventKind::ImpromptuResolved,
            Self::SkillEffectStarted(_) => EventKind::SkillEffectStarted,
            Self::SkillAction(_) => EventKind::SkillAction,
            Self::AllyAction(_) => EventKind::AllyAction,
            Self::BuffAdded(_) => EventKind::BuffAdded,
            Self::BuffChanged(_) => EventKind::BuffChanged,
            Self::BuffRemoved(_) => EventKind::BuffRemoved,
            Self::BuffsSettled(_) => EventKind::RoundEndFinalSettlement,
            Self::BuffFeatureTriggered(_) => EventKind::BuffFeatureTriggered,
            Self::HpLost { .. } => EventKind::HpLost,
            Self::HpHealed { .. } => EventKind::HpHealed,
            Self::ToughnessBroken { .. } => EventKind::ToughnessBroken,
            Self::Hit(_) => EventKind::TargetAttacked,
            Self::EntityDied(_) => EventKind::EntityDied,
            Self::BattleTerminalCommitted { .. } => EventKind::BattleTerminalCommitted,
            Self::ExPointChanged(_) => EventKind::ExPointChanged,
            Self::ExPointOverflow(_) => EventKind::ExPointOverflow,
            Self::EurekaChanged(_) => EventKind::EurekaChanged,
            Self::ConduitActivated(_) => EventKind::ConduitActivated,
            Self::GaugeChanged(_) => EventKind::GaugeChanged,
            Self::CardChanged(_) => EventKind::CardChanged,
            Self::FieldChanged(_) => EventKind::FieldChanged,
            Self::SummonChanged(_) => EventKind::SummonChanged,
            Self::ShellChanged(change) => match change.kind {
                crate::engine::mechanic::shell::ShellChangeKind::Deployed => {
                    EventKind::ShellDeployed
                }
                crate::engine::mechanic::shell::ShellChangeKind::Retrieved => {
                    EventKind::ShellRetrieved
                }
            },
            Self::BloodtitheChanged { .. } => EventKind::BloodtitheChanged,
            Self::Kind(kind) => *kind,
        }
    }

    pub fn subscription_kinds(&self) -> impl Iterator<Item = EventKind> {
        let publishes_target_attacked = !matches!(self, Self::Hit(_))
            || matches!(
                self,
                Self::Hit(HitEvent {
                    damage_from: crate::engine::manager::hp::HurtDamageFromType::Skill,
                    ..
                })
            );
        [
            publishes_target_attacked.then_some(self.kind()),
            matches!(self, Self::Hit(_)).then_some(EventKind::BeAttacked),
            matches!(self, Self::Hit(_)).then_some(EventKind::Riposte),
            matches!(
                self,
                Self::SkillAction(crate::engine::skill::action::SkillActionEvent {
                    phase: crate::engine::skill::action::SkillPhase::HitPassives,
                    ..
                })
            )
            .then_some(EventKind::SkillCast),
        ]
        .into_iter()
        .flatten()
    }
}

#[cfg(test)]
mod subscription_tests {
    use super::*;
    use crate::engine::{
        manager::hp::HurtDamageFromType,
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    fn hit(damage_from: HurtDamageFromType) -> BattleEvent {
        BattleEvent::Hit(HitEvent {
            origin: CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(1, "SkillDamage"),
            },
            source_uid: 1,
            target_uid: -1,
            skill_id: 1,
            amount: 1,
            damage_from,
            assassinate: false,
        })
    }

    #[test]
    fn only_primary_skill_damage_publishes_target_attacked() {
        assert_eq!(
            hit(HurtDamageFromType::Skill)
                .subscription_kinds()
                .collect::<Vec<_>>(),
            vec![
                EventKind::TargetAttacked,
                EventKind::BeAttacked,
                EventKind::Riposte,
            ]
        );
        assert_eq!(
            hit(HurtDamageFromType::Additional)
                .subscription_kinds()
                .collect::<Vec<_>>(),
            vec![EventKind::BeAttacked, EventKind::Riposte]
        );
    }

    #[test]
    fn skill_cast_uses_the_hit_passives_phase_not_action_completion() {
        let action = crate::engine::skill::action::SkillActionEvent {
            source_uid: 1,
            skill_id: 2,
            target_uid: -1,
            target_uids: vec![-1],
            attacked_target_uids: vec![-1],
            phase: crate::engine::skill::action::SkillPhase::HitPassives,
            skill_slot: 1,
            is_attack: true,
            rank: 1,
            skill_type: 0,
            effect_tag: 1,
            assassinate: false,
            damage_amount: 10,
            kill_count: 0,
            crit_count: 0,
            guard_break_count: 0,
            additional_moxie: 0,
            extra_skill_kind: 0,
            mode: crate::engine::skill::action::SkillExecutionMode::Active,
            teammate_injury_count: 0,
            teammate_injury_count_not_reset: 0,
            team_injury_count_round: 0,
            card_enchants: Vec::new(),
        };

        assert_eq!(
            BattleEvent::SkillAction(action)
                .subscription_kinds()
                .collect::<Vec<_>>(),
            vec![EventKind::SkillAction, EventKind::SkillCast]
        );
        assert_eq!(
            BattleEvent::AllyAction(Default::default())
                .subscription_kinds()
                .collect::<Vec<_>>(),
            vec![EventKind::AllyAction]
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buff_changes_keep_committed_identity_and_amounts() {
        let change = BuffChangeEvent {
            source_uid: 10,
            target_uid: -1,
            buff_uid: -20,
            buff_id: 30,
            before_amount: 1,
            after_amount: 2,
            act_id: 0,
            act_value: 0,
        };

        assert_eq!(BattleEvent::BuffAdded(change).kind(), EventKind::BuffAdded);
        assert_eq!(
            BattleEvent::BuffChanged(change).kind(),
            EventKind::BuffChanged
        );
        assert_eq!(
            BattleEvent::BuffRemoved(change).kind(),
            EventKind::BuffRemoved
        );
        assert_eq!(change.after_amount, 2);
    }
}
