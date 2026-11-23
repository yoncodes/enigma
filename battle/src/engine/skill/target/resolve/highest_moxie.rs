use crate::engine::{
    manager::{BattleManagers, ex_point::ExPointKind},
    skill::target::TargetEntity,
};

fn excluded_model(args: &[i32]) -> Option<Option<i32>> {
    match args {
        [] => Some(None),
        [model] if *model > 0 => Some(Some(*model)),
        _ => None,
    }
}

pub(super) fn supports(args: &[i32]) -> bool {
    excluded_model(args).is_some()
}

pub(super) fn select(
    args: &[i32],
    allies: &[TargetEntity],
    source_uid: i64,
    managers: Option<&BattleManagers>,
    selected: Option<i64>,
) -> Vec<i64> {
    let Some(excluded) = excluded_model(args) else {
        return Vec::new();
    };
    allies
        .iter()
        .filter(|entity| {
            excluded != Some(entity.model_id)
                && managers.is_none_or(|managers| {
                    managers.ex_point.kind(entity.uid) == ExPointKind::Common.as_wire()
                })
        })
        .min_by_key(|entity| {
            (
                -managers.map_or(entity.ex_point, |managers| {
                    managers.ex_point.get(entity.uid)
                }),
                selected != Some(entity.uid),
                entity.position,
                entity.uid,
            )
        })
        .map(|entity| entity.uid)
        .or_else(|| excluded.is_none().then_some(source_uid))
        .into_iter()
        .collect()
}
