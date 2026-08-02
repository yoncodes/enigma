use std::collections::HashSet;

use super::{BattleRuntime, drain::DrainResult, executor::RuleOutcome};
use crate::engine::{
    fight::reserve::Promotion,
    round::command::RoundCommand,
    skill::{
        action::{SkillExecutionMode, SkillLifecycle},
        effect::{SkillEffectCatalog, catalog::SkillEffectTag},
    },
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct FinishingAction {
    rank: i32,
    kill_count: i32,
    is_ultimate: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ObjectiveProgress {
    promoted_attacker_casualties: HashSet<i64>,
    moved_incantation: bool,
    used_tuning: bool,
    used_healing_incantation: bool,
    used_debuff_incantation: bool,
    max_ultimates_in_round: i32,
    max_incantation_damage: i32,
    finishing_action: Option<FinishingAction>,
}

impl ObjectiveProgress {
    pub(super) fn record_promotions(&mut self, promotions: &[Promotion]) {
        self.promoted_attacker_casualties.extend(
            promotions
                .iter()
                .filter(|promotion| promotion.team_type == 1)
                .map(|promotion| promotion.defeated_uid),
        );
    }

    pub(super) fn record_player_round(
        &mut self,
        commands: &[RoundCommand],
        catalog: &SkillEffectCatalog,
        result: &DrainResult,
        ended_by_player: bool,
    ) {
        self.moved_incantation |= commands
            .iter()
            .any(|command| matches!(command, RoundCommand::MoveCard { .. }));

        let mut ultimates = 0;
        let mut terminal_action = None;
        for outcome in &result.outcomes {
            if outcome.death_count() > 0 {
                terminal_action = None;
            }
            let RuleOutcome::SkillLifecycle(SkillLifecycle::ActionCompleted(action)) = outcome
            else {
                continue;
            };
            terminal_action = Some(action);
            if action.mode != SkillExecutionMode::Active {
                continue;
            }
            ultimates += i32::from(catalog.is_big_skill(action.skill_id));
            self.used_healing_incantation |= action.effect_tag == SkillEffectTag::Heal as i32;
            self.used_debuff_incantation |= action.effect_tag == SkillEffectTag::Debuff as i32;
            self.max_incantation_damage = self.max_incantation_damage.max(action.damage_amount);
        }
        self.max_ultimates_in_round = self.max_ultimates_in_round.max(ultimates);
        self.finishing_action = terminal_action
            .filter(|action| {
                ended_by_player
                    && action.mode == SkillExecutionMode::Active
                    && action.kill_count > 0
            })
            .map(|action| FinishingAction {
                rank: action.rank,
                kill_count: action.kill_count,
                is_ultimate: catalog.is_big_skill(action.skill_id),
            });
    }

    pub(super) fn record_tuning_use(&mut self) {
        self.used_tuning = true;
    }
}

#[derive(Clone, Copy)]
#[repr(i32)]
enum AdvancedConditionType {
    CasualtiesBelow = 1,
    RoundsAtMost = 2,
    NoCasualtiesWithinRounds = 3,
    FinishingIncantation = 4,
    FinalHitKills = 5,
    ForbiddenAction = 6,
    UltimatesInOneRound = 7,
    IncantationDamage = 8,
    AverageHp = 9,
}

impl AdvancedConditionType {
    fn from_id(id: i32) -> Option<Self> {
        Some(match id {
            1 => Self::CasualtiesBelow,
            2 => Self::RoundsAtMost,
            3 => Self::NoCasualtiesWithinRounds,
            4 => Self::FinishingIncantation,
            5 => Self::FinalHitKills,
            6 => Self::ForbiddenAction,
            7 => Self::UltimatesInOneRound,
            8 => Self::IncantationDamage,
            9 => Self::AverageHp,
            _ => return None,
        })
    }
}

struct ObjectiveContext {
    dead_attackers: i32,
    current_round: i32,
    average_hp_ratio: f64,
}

fn condition_met(
    progress: &ObjectiveProgress,
    condition_type: AdvancedConditionType,
    attr: i32,
    context: ObjectiveContext,
) -> bool {
    match condition_type {
        AdvancedConditionType::CasualtiesBelow => context.dead_attackers < attr,
        AdvancedConditionType::RoundsAtMost => context.current_round <= attr,
        AdvancedConditionType::NoCasualtiesWithinRounds => {
            context.dead_attackers == 0 && context.current_round <= attr
        }
        AdvancedConditionType::FinishingIncantation => {
            progress.finishing_action.is_some_and(|action| match attr {
                1 => action.is_ultimate,
                2 => action.rank == 3,
                _ => false,
            })
        }
        AdvancedConditionType::FinalHitKills => progress
            .finishing_action
            .is_some_and(|action| action.kill_count >= attr),
        AdvancedConditionType::ForbiddenAction => match attr {
            1 => !progress.moved_incantation,
            2 => !progress.used_tuning,
            3 => !progress.used_healing_incantation,
            4 => !progress.used_debuff_incantation,
            _ => false,
        },
        AdvancedConditionType::UltimatesInOneRound => progress.max_ultimates_in_round >= attr,
        AdvancedConditionType::IncantationDamage => progress.max_incantation_damage > attr,
        AdvancedConditionType::AverageHp => context.average_hp_ratio * 1_000.0 > f64::from(attr),
    }
}

impl BattleRuntime {
    pub fn meets_advanced_condition(&self, type_id: i32, attr: i32) -> Option<bool> {
        let condition_type = AdvancedConditionType::from_id(type_id)?;
        Some(condition_met(
            &self.objectives,
            condition_type,
            attr,
            ObjectiveContext {
                dead_attackers: self.dead_attacker_count() as i32
                    + self.objectives.promoted_attacker_casualties.len() as i32,
                current_round: self.current_round(),
                average_hp_ratio: self.average_attacker_hp_ratio(),
            },
        ))
    }

    fn average_attacker_hp_ratio(&self) -> f64 {
        let Some(team) = self.fight.attacker.as_ref() else {
            return 0.0;
        };
        let entities = &team.entitys;
        if entities.is_empty() {
            return 0.0;
        }
        let total = entities
            .iter()
            .filter_map(|entity| {
                let uid = entity.uid?;
                let hp = self.managers.hp.get(uid);
                (hp.current > 0 && hp.max > 0).then_some(hp.current as f64 / hp.max as f64)
            })
            .sum::<f64>();
        total / (entities.len() + self.objectives.promoted_attacker_casualties.len()) as f64
    }
}

#[cfg(test)]
mod test;
