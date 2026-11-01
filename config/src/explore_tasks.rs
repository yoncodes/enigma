//! Exploration collection tasks from the matching client configuration.

use serde::Deserialize;
use std::{collections::HashSet, sync::OnceLock};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: i32,
    pub is_online: i32,
    pub listener_type: String,
    pub listener_param: String,
    pub max_progress: i32,
    pub bonus: String,
}

impl Task {
    pub fn collection(&self) -> anyhow::Result<(i32, i32)> {
        anyhow::ensure!(self.listener_type == "ExploreCollect", "unsupported exploration task listener");
        let mut parts = self.listener_param.split('#');
        let chapter = parts.next().ok_or_else(|| anyhow::anyhow!("missing task chapter"))?.parse()?;
        let coin = parts.next().ok_or_else(|| anyhow::anyhow!("missing task coin type"))?.parse()?;
        anyhow::ensure!(parts.next().is_none() && matches!(coin, 1 | 2) && self.max_progress > 0,
            "invalid exploration task parameters");
        Ok((chapter, coin))
    }
}

static TASKS: OnceLock<Result<Vec<Task>, String>> = OnceLock::new();

pub fn tasks() -> anyhow::Result<&'static [Task]> {
    match TASKS.get_or_init(|| {
        let tasks: Vec<Task> = serde_json::from_str(include_str!("../data/explore_tasks.json"))
            .map_err(|error| format!("invalid exploration tasks: {error}"))?;
        let mut ids = HashSet::new();
        for task in &tasks {
            if !ids.insert(task.id) { return Err("duplicate exploration task id".into()); }
            task.collection().map_err(|error| error.to_string())?;
        }
        Ok(tasks)
    }) {
        Ok(tasks) => Ok(tasks),
        Err(error) => Err(anyhow::anyhow!("{error}")),
    }
}

pub fn task(id: i32) -> anyhow::Result<Option<&'static Task>> {
    Ok(tasks()?.iter().find(|task| task.id == id))
}
