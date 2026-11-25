use super::{FightTask, TaskStatus};
use anyhow::{Context, Result, ensure};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub(super) struct TaskPrerequisites {
    by_task: HashMap<i32, Vec<i32>>,
}

impl TaskPrerequisites {
    pub(super) fn insert(&mut self, task_id: i32, condition: &str) -> Result<()> {
        let required = parse(condition)?;
        ensure!(!required.contains(&task_id), "task cannot require itself");
        if !required.is_empty() {
            self.by_task.insert(task_id, required);
        }
        Ok(())
    }

    pub(super) fn validate(&self, task_ids: &[i32], tasks: &[FightTask]) -> Result<()> {
        let known = task_ids.iter().copied().collect::<HashSet<_>>();
        ensure!(known.len() == task_ids.len(), "duplicate fight task id");
        ensure!(
            self.by_task.values().flatten().all(|id| known.contains(id)),
            "fight task condition references a missing prerequisite"
        );

        let mut reachable = HashSet::new();
        loop {
            let previous_count = reachable.len();
            for &id in task_ids {
                if self.ready(id, &reachable) {
                    reachable.insert(id);
                }
            }
            if reachable.len() == previous_count {
                break;
            }
        }
        ensure!(
            reachable.len() == known.len(),
            "fight task prerequisites contain a cycle"
        );

        let finished = finished(tasks);
        for (&id, task) in task_ids.iter().zip(tasks) {
            let expected = if !self.ready(id, &finished) {
                ensure!(
                    task.values.iter().all(|value| value.progress == Some(0)),
                    "waiting fight task {id} has progress"
                );
                TaskStatus::Waiting
            } else {
                TaskStatus::from_progress(&task.values)
            };
            ensure!(
                task.status == expected.wire(),
                "fight task {id} status differs from progress or prerequisites"
            );
        }
        Ok(())
    }

    pub(super) fn activate(&self, tasks: &mut [FightTask]) -> Vec<i32> {
        if self.by_task.is_empty() {
            return Vec::new();
        }
        let finished = finished(tasks);
        let mut activated = Vec::new();
        for task in tasks {
            let Some(id) = task.task_id else {
                continue;
            };
            if task.status == TaskStatus::Waiting.wire() && self.ready(id, &finished) {
                task.status = TaskStatus::Running.wire();
                activated.push(id);
            }
        }
        activated
    }

    fn ready(&self, id: i32, finished: &HashSet<i32>) -> bool {
        self.by_task
            .get(&id)
            .is_none_or(|required| required.iter().all(|id| finished.contains(id)))
    }
}

fn finished(tasks: &[FightTask]) -> HashSet<i32> {
    tasks
        .iter()
        .filter(|task| task.status == TaskStatus::Finished.wire())
        .filter_map(|task| task.task_id)
        .collect()
}

pub(super) fn parse(condition: &str) -> Result<Vec<i32>> {
    if condition.is_empty() {
        return Ok(Vec::new());
    }
    let raw = condition
        .strip_prefix("FinishedTask#")
        .with_context(|| format!("unsupported fight task condition {condition:?}"))?;
    let mut unique = HashSet::new();
    raw.split(',')
        .map(|raw_id| {
            let id = raw_id
                .parse()
                .with_context(|| format!("invalid fight task prerequisite {raw_id:?}"))?;
            ensure!(id > 0, "fight task prerequisite must be positive");
            ensure!(unique.insert(id), "duplicate fight task prerequisite {id}");
            Ok(id)
        })
        .collect()
}
