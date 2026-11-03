use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result, bail, ensure};

use sonettobuf::{Fight, FightTask, FightTaskBox, FightTaskValue};

const RUNNING: i32 = 2;
const FINISHED: i32 = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
enum TaskProgress {
    IndicatorValue(i32),
    IndicatorThreshold { indicator_id: i32, threshold: i32 },
    KillMonster(Vec<i32>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskRule {
    value_index: i32,
    progress: TaskProgress,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FightTaskChange {
    pub team_type: i32,
    pub tasks: Vec<FightTask>,
}

#[derive(Debug, Clone, Default)]
pub struct FightTaskManager {
    tasks: Vec<FightTask>,
    rules: HashMap<i32, Vec<TaskRule>>,
    models: HashMap<i64, i32>,
    pending: HashSet<i32>,
}

impl FightTaskManager {
    pub fn seed(catalog: crate::catalog::BattleCatalog, fight: &Fight) -> Result<Self> {
        let tasks = fight
            .fight_task_box
            .as_ref()
            .map(|task_box| task_box.tasks.clone())
            .unwrap_or_default();
        let actual_ids = tasks
            .iter()
            .map(|task| {
                task.task_id
                    .context("fight task snapshot contains a task without an id")
            })
            .collect::<Result<Vec<_>>>()?;
        let episode_id = fight.episode_id.unwrap_or_default();
        if catalog
            .game_data()
            .teaching_episode
            .get(episode_id)
            .is_some()
        {
            let expected_ids = configured_box(catalog, episode_id)?
                .tasks
                .into_iter()
                .filter_map(|task| task.task_id)
                .collect::<Vec<_>>();
            ensure!(
                actual_ids == expected_ids,
                "fight task snapshot sequence differs from episode config"
            );
        }
        let mut rules = HashMap::new();
        for (task, task_id) in tasks.iter().zip(actual_ids) {
            let row = catalog
                .game_data()
                .fight_task
                .get(task_id)
                .with_context(|| {
                    format!("fight task snapshot references missing config {task_id}")
                })?;
            let definitions =
                task_rules(row).with_context(|| format!("unsupported fight task {task_id}"))?;
            validate_snapshot(task, &definitions)
                .with_context(|| format!("invalid fight task snapshot {task_id}"))?;
            rules.insert(
                task_id,
                definitions.into_iter().map(|(rule, _)| rule).collect(),
            );
        }
        Ok(Self {
            tasks,
            rules,
            models: super::entities(fight)
                .filter_map(|entity| Some((entity.uid?, entity.model_id?)))
                .collect(),
            pending: HashSet::new(),
        })
    }

    pub fn record_indicator(&mut self, indicator_id: i32, value: i32) {
        for task in &mut self.tasks {
            let Some(task_id) = task.task_id else {
                continue;
            };
            if task.status == Some(FINISHED) {
                continue;
            }
            let Some(definitions) = self.rules.get(&task_id) else {
                continue;
            };
            let mut changed = false;
            for definition in definitions {
                let Some(state) = task
                    .values
                    .iter_mut()
                    .find(|state| state.index == Some(definition.value_index))
                else {
                    continue;
                };
                let max = state.max_progress.unwrap_or_default();
                if max <= 0 {
                    continue;
                }
                let progress = match definition.progress {
                    TaskProgress::IndicatorValue(id) if id == indicator_id => value.clamp(0, max),
                    TaskProgress::IndicatorThreshold {
                        indicator_id: id,
                        threshold,
                    } if id == indicator_id => {
                        if value >= threshold {
                            max
                        } else {
                            0
                        }
                    }
                    _ => continue,
                };
                let finished = progress >= max;
                changed |= state.progress != Some(progress) || state.finished != Some(finished);
                state.progress = Some(progress);
                state.finished = Some(finished);
            }
            if !changed {
                continue;
            }
            task.status = Some(
                if task.values.iter().all(|value| value.finished == Some(true)) {
                    FINISHED
                } else {
                    RUNNING
                },
            );
            self.pending.insert(task_id);
        }
    }

    pub fn record_kill(&mut self, target_uid: i64, team_type: i32) -> Option<FightTaskChange> {
        let model_id = *self.models.get(&target_uid)?;
        let mut changed_tasks = Vec::new();
        for task in &mut self.tasks {
            let Some(task_id) = task.task_id else {
                continue;
            };
            if task.status == Some(FINISHED) {
                continue;
            }
            let Some(definitions) = self.rules.get(&task_id) else {
                continue;
            };
            let mut changed = false;
            for definition in definitions {
                let TaskProgress::KillMonster(model_ids) = &definition.progress else {
                    continue;
                };
                if !model_ids.contains(&model_id) {
                    continue;
                }
                let Some(state) = task
                    .values
                    .iter_mut()
                    .find(|state| state.index == Some(definition.value_index))
                else {
                    continue;
                };
                let max = state.max_progress.unwrap_or_default();
                if max <= 0 {
                    continue;
                }
                let progress = state
                    .progress
                    .unwrap_or_default()
                    .saturating_add(1)
                    .min(max);
                let finished = progress >= max;
                changed |= state.progress != Some(progress) || state.finished != Some(finished);
                state.progress = Some(progress);
                state.finished = Some(finished);
            }
            if !changed {
                continue;
            }
            task.status = Some(
                if task.values.iter().all(|value| value.finished == Some(true)) {
                    FINISHED
                } else {
                    RUNNING
                },
            );
            changed_tasks.push(task.clone());
        }
        (!changed_tasks.is_empty()).then_some(FightTaskChange {
            team_type,
            tasks: changed_tasks,
        })
    }

    pub fn take_pending(&mut self, team_type: i32) -> Option<FightTaskChange> {
        if self.pending.is_empty() {
            return None;
        }
        let tasks = self
            .tasks
            .iter()
            .filter(|task| {
                task.task_id
                    .is_some_and(|task_id| self.pending.contains(&task_id))
            })
            .cloned()
            .collect();
        self.pending.clear();
        Some(FightTaskChange { team_type, tasks })
    }

    pub fn sync_fight(&self, fight: &mut Fight) {
        fight.fight_task_box = Some(FightTaskBox {
            tasks: self.tasks.clone(),
        });
    }
}

pub(crate) fn configured_box(
    catalog: crate::catalog::BattleCatalog,
    episode_id: i32,
) -> Result<FightTaskBox> {
    let game = catalog.game_data();
    let Some(episode) = game.teaching_episode.get(episode_id) else {
        return Ok(FightTaskBox::default());
    };
    if episode.battle_tasks.trim().is_empty() {
        return Ok(FightTaskBox::default());
    }
    let mut rows = Vec::new();
    for (index, raw_task_id) in episode.battle_tasks.split('|').enumerate() {
        let task_id = raw_task_id.parse::<i32>().with_context(|| {
            format!("teaching episode {episode_id} has invalid fight task id '{raw_task_id}'")
        })?;
        let row = game.fight_task.get(task_id).with_context(|| {
            format!("teaching episode {episode_id} references missing fight task {task_id}")
        })?;
        let rules = task_rules(row).with_context(|| format!("invalid fight task {task_id}"))?;
        rows.push((index, task_id, rules));
    }
    // Captured tutorials keep the primary objective first, then defeat, then auxiliaries.
    rows.sort_by_key(|(index, _, rules)| {
        if *index == 0 {
            0
        } else if rules
            .iter()
            .any(|(rule, _)| matches!(rule.progress, TaskProgress::KillMonster(_)))
        {
            1
        } else {
            2
        }
    });
    let tasks = rows
        .into_iter()
        .map(|(_, task_id, rules)| {
            let values = rules
                .into_iter()
                .map(|(rule, max_progress)| FightTaskValue {
                    index: Some(rule.value_index),
                    progress: Some(0),
                    max_progress: Some(max_progress),
                    finished: Some(false),
                })
                .collect();
            FightTask {
                task_id: Some(task_id),
                status: Some(RUNNING),
                values,
            }
        })
        .collect();
    Ok(FightTaskBox { tasks })
}

fn task_rules(row: &config::fight_task::FightTask) -> Result<Vec<(TaskRule, i32)>> {
    let mut rules = Vec::new();
    for (index, (parameter, raw_max)) in task_params(row).into_iter().enumerate() {
        if parameter.is_empty() && raw_max.is_empty() {
            continue;
        }
        ensure!(
            !parameter.is_empty() && !raw_max.is_empty(),
            "task parameter {} is incomplete",
            index + 1
        );
        let max_progress = raw_max
            .parse::<i32>()
            .with_context(|| format!("invalid maximum progress '{raw_max}'"))?;
        ensure!(max_progress > 0, "maximum progress must be positive");
        rules.push((parse_task_rule(index as i32 + 1, parameter)?, max_progress));
    }
    ensure!(!rules.is_empty(), "task has no progress rules");
    Ok(rules)
}

fn validate_snapshot(task: &FightTask, rules: &[(TaskRule, i32)]) -> Result<()> {
    ensure!(
        task.values.len() == rules.len(),
        "progress value count differs"
    );
    let mut all_finished = true;
    for (rule, expected_max) in rules {
        let matches = task
            .values
            .iter()
            .filter(|value| value.index == Some(rule.value_index))
            .collect::<Vec<_>>();
        ensure!(
            matches.len() == 1,
            "progress index {} is invalid",
            rule.value_index
        );
        let value = matches[0];
        ensure!(
            value.max_progress == Some(*expected_max),
            "progress maximum differs at index {}",
            rule.value_index
        );
        let progress = value
            .progress
            .with_context(|| format!("progress is missing at index {}", rule.value_index))?;
        ensure!(
            (0..=*expected_max).contains(&progress),
            "progress is out of range at index {}",
            rule.value_index
        );
        let finished = progress >= *expected_max;
        ensure!(
            value.finished == Some(finished),
            "finished state differs at index {}",
            rule.value_index
        );
        all_finished &= finished;
    }
    ensure!(
        task.status == Some(if all_finished { FINISHED } else { RUNNING }),
        "task status differs from progress"
    );
    Ok(())
}

fn task_params(row: &config::fight_task::FightTask) -> [(&str, &str); 4] {
    [
        (&row.task_param1, &row.sys_param1),
        (&row.task_param2, &row.sys_param2),
        (&row.task_param3, &row.sys_param3),
        (&row.task_param4, &row.sys_param4),
    ]
}

fn parse_task_rule(value_index: i32, parameter: &str) -> Result<TaskRule> {
    let mut parts = parameter.split('#');
    let kind = parts.next().unwrap_or_default();
    let progress = match kind {
        "IndicatorNum" => {
            let indicator_id = parse_i32(parts.next(), "indicator id")?;
            ensure!(parts.next().is_none(), "IndicatorNum has extra arguments");
            TaskProgress::IndicatorValue(indicator_id)
        }
        "Indicator" => {
            let indicator_id = parse_i32(parts.next(), "indicator id")?;
            let threshold = parse_i32(parts.next(), "indicator threshold")?;
            ensure!(
                parts.next() == Some("4"),
                "unsupported Indicator comparator"
            );
            ensure!(parts.next().is_none(), "Indicator has extra arguments");
            TaskProgress::IndicatorThreshold {
                indicator_id,
                threshold,
            }
        }
        "KillMonster" => {
            let raw_ids = parts.next().context("KillMonster is missing model ids")?;
            ensure!(parts.next().is_none(), "KillMonster has extra arguments");
            let model_ids = raw_ids
                .split(',')
                .map(|raw| {
                    raw.parse::<i32>()
                        .with_context(|| format!("invalid monster model id '{raw}'"))
                })
                .collect::<Result<Vec<_>>>()?;
            ensure!(!model_ids.is_empty(), "KillMonster has no model ids");
            TaskProgress::KillMonster(model_ids)
        }
        _ => bail!("unsupported fight task kind '{kind}'"),
    };
    Ok(TaskRule {
        value_index,
        progress,
    })
}

fn parse_i32(value: Option<&str>, name: &str) -> Result<i32> {
    let raw = value.with_context(|| format!("missing {name}"))?;
    raw.parse()
        .with_context(|| format!("invalid {name} '{raw}'"))
}

#[cfg(test)]
mod tests;
