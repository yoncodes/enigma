use super::*;

pub async fn act172_info(
    db: &SqlitePool,
    player_id: i64,
    activity_id: Option<i32>,
) -> Result<GetAct172InfoReply, AppError> {
    let activity_id = activity_id
        .or_else(|| {
            config::configs::get()
                .activity172_task
                .iter()
                .map(|row| row.activity_id)
                .max()
        })
        .ok_or(AppError::InvalidRequest)?;
    let states = activity_state::get(
        db,
        player_id,
        activity_id,
        ActivityStateKind::Act172UseItemTask,
    )
    .await?;
    let mut use_item_task_ids = states
        .into_iter()
        .filter_map(|(task_id, (state, _, _))| (state != 0).then_some(task_id))
        .collect::<Vec<_>>();
    use_item_task_ids.sort_unstable();

    Ok(GetAct172InfoReply {
        activity_id: Some(activity_id),
        info: Some(Act172Info { use_item_task_ids }),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn default_act172_activity_id_comes_from_latest_config() {
        let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
        let _ = config::init(&data_dir);
        let activity_id = config::configs::get()
            .activity172_task
            .iter()
            .map(|row| row.activity_id)
            .max()
            .unwrap();

        assert_eq!(activity_id, 13922);
    }
}
