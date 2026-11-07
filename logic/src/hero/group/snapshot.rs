use super::HeroManager;
use crate::{error::AppError, types::hero_group_snapshot_type::HeroGroupSnapshotType};
use database::{
    db::game::{hero_group_snapshots, hero_groups},
    models::game::{hero_group_snapshots::HeroGroupSnapshotInfo, hero_groups as model},
};
use sonettobuf::{
    FightGroup, GetHeroGroupSnapshotListReply, HeroGroupSnapshotFightGroupNo,
    HeroGroupSnapshotGroupInfoNo, SetHeroGroupSnapshotBatchReply, SetHeroGroupSnapshotReply,
};
use sqlx::SqlitePool;
use std::collections::{HashMap, HashSet};

impl HeroManager {
    pub async fn group_snapshot_list(
        &self,
        db: &SqlitePool,
        snapshot_id: Option<i32>,
    ) -> Result<GetHeroGroupSnapshotListReply, AppError> {
        let snapshots = match snapshot_id.unwrap_or_default() {
            id if id > 0 => {
                HeroGroupSnapshotType::from_id(id).ok_or(AppError::InvalidRequest)?;
                let saved =
                    hero_group_snapshots::get_hero_group_snapshot(db, self.player_id, id).await?;
                vec![if id == HeroGroupSnapshotType::Common.id() {
                    common_snapshot(db, self.player_id, saved).await?
                } else {
                    saved.unwrap_or_else(|| empty_snapshot(id))
                }]
            }
            0 => {
                let mut saved =
                    hero_group_snapshots::get_hero_group_snapshots(db, self.player_id).await?;
                let common_index = saved.iter().position(|snapshot| {
                    snapshot.snapshot_id == HeroGroupSnapshotType::Common.id()
                });
                let stored_common = common_index.map(|index| saved.remove(index));
                saved.push(common_snapshot(db, self.player_id, stored_common).await?);
                overlay_snapshot_catalog(saved)
            }
            _ => return Err(AppError::InvalidRequest),
        };

        Ok(GetHeroGroupSnapshotListReply {
            hero_group_snapshots: snapshots.into_iter().map(Into::into).collect(),
        })
    }
}

async fn common_snapshot(
    db: &SqlitePool,
    player_id: i64,
    stored: Option<HeroGroupSnapshotInfo>,
) -> Result<HeroGroupSnapshotInfo, AppError> {
    let groups = hero_groups::get_hero_groups_common(db, player_id).await?;
    let sort_sub_ids = stored
        .map(|snapshot| snapshot.sort_sub_ids)
        .filter(|ids| !ids.is_empty())
        .unwrap_or_else(|| groups.iter().map(|group| group.group_id).collect());

    Ok(HeroGroupSnapshotInfo {
        snapshot_id: HeroGroupSnapshotType::Common.id(),
        hero_group_snapshots: groups,
        sort_sub_ids,
    })
}

fn empty_snapshot(snapshot_id: i32) -> HeroGroupSnapshotInfo {
    HeroGroupSnapshotInfo {
        snapshot_id,
        hero_group_snapshots: Vec::new(),
        sort_sub_ids: Vec::new(),
    }
}

pub(super) fn overlay_snapshot_catalog(
    snapshots: Vec<HeroGroupSnapshotInfo>,
) -> Vec<HeroGroupSnapshotInfo> {
    let mut saved = snapshots
        .into_iter()
        .map(|snapshot| (snapshot.snapshot_id, snapshot))
        .collect::<HashMap<_, _>>();

    HeroGroupSnapshotType::ALL_DESCENDING
        .into_iter()
        .map(|snapshot_type| {
            let id = snapshot_type.id();
            saved.remove(&id).unwrap_or_else(|| empty_snapshot(id))
        })
        .collect()
}

impl HeroManager {
    pub async fn set_group_snapshot_batch(
        &self,
        db: &SqlitePool,
        snapshot_id: i32,
        fight_groups: Vec<HeroGroupSnapshotFightGroupNo>,
    ) -> Result<SetHeroGroupSnapshotBatchReply, AppError> {
        if HeroGroupSnapshotType::from_id(snapshot_id).is_none() || fight_groups.is_empty() {
            return Err(AppError::InvalidRequest);
        }
        let mut sub_ids = Vec::with_capacity(fight_groups.len());
        let mut groups = Vec::with_capacity(fight_groups.len());
        let mut seen = HashSet::with_capacity(fight_groups.len());
        for group in fight_groups {
            let sub_id = group.snapshot_sub_id.ok_or(AppError::InvalidRequest)?;
            if sub_id <= 0 || !seen.insert(sub_id) {
                return Err(AppError::InvalidRequest);
            }
            sub_ids.push(sub_id);
            groups.push(snapshot_group(
                sub_id,
                group.fight_group.ok_or(AppError::InvalidRequest)?,
            ));
        }

        if snapshot_id == HeroGroupSnapshotType::Common.id() {
            let mut tx = db.begin().await?;
            for group in &groups {
                hero_group_snapshots::save_common_group_snapshot_in_transaction(
                    &mut tx,
                    self.player_id,
                    snapshot_id,
                    group,
                )
                .await?;
            }
            tx.commit().await?;
        } else {
            hero_group_snapshots::save_hero_group_snapshot(
                db,
                self.player_id,
                snapshot_id,
                groups.clone(),
                sub_ids.clone(),
            )
            .await?;
        }

        Ok(SetHeroGroupSnapshotBatchReply {
            snapshot_id: Some(snapshot_id),
            group_infos: sub_ids
                .into_iter()
                .zip(groups)
                .map(|(snapshot_sub_id, group)| HeroGroupSnapshotGroupInfoNo {
                    snapshot_sub_id: Some(snapshot_sub_id),
                    group_info: Some(group.into()),
                })
                .collect(),
        })
    }

    pub async fn set_group_snapshot(
        &self,
        db: &SqlitePool,
        snapshot_id: i32,
        snapshot_sub_id: i32,
        fight_group: FightGroup,
    ) -> Result<SetHeroGroupSnapshotReply, AppError> {
        if HeroGroupSnapshotType::from_id(snapshot_id).is_none() || snapshot_sub_id <= 0 {
            return Err(AppError::InvalidRequest);
        }
        let hero_group = snapshot_group(snapshot_sub_id, fight_group);

        if snapshot_id == HeroGroupSnapshotType::Common.id() {
            let mut tx = db.begin().await?;
            hero_group_snapshots::save_common_group_snapshot_in_transaction(
                &mut tx,
                self.player_id,
                snapshot_id,
                &hero_group,
            )
            .await?;
            tx.commit().await?;
        } else {
            hero_group_snapshots::save_hero_group_snapshot(
                db,
                self.player_id,
                snapshot_id,
                vec![hero_group.clone()],
                vec![snapshot_sub_id],
            )
            .await?;
        }

        Ok(SetHeroGroupSnapshotReply {
            snapshot_id: Some(snapshot_id),
            snapshot_sub_id: Some(snapshot_sub_id),
            group_info: Some(hero_group.into()),
        })
    }
}

pub(super) fn snapshot_group(
    snapshot_sub_id: i32,
    fight_group: FightGroup,
) -> model::HeroGroupInfo {
    model::HeroGroupInfo {
        group_id: snapshot_sub_id,
        hero_list: fight_group
            .hero_list
            .into_iter()
            .chain(fight_group.sub_hero_list)
            .collect(),
        name: String::new(),
        cloth_id: fight_group.cloth_id.unwrap_or(1),
        equips: fight_group
            .equips
            .into_iter()
            .enumerate()
            .map(|(index, equip)| model::HeroGroupEquip {
                index: index as i32,
                equip_uids: equip.equip_uid,
            })
            .collect(),
        activity104_equips: fight_group
            .activity104_equips
            .into_iter()
            .enumerate()
            .map(|(index, equip)| model::HeroGroupEquip {
                index: index as i32,
                equip_uids: equip.equip_uid,
            })
            .collect(),
        assist_boss_id: fight_group.assist_boss_id.unwrap_or_default(),
        params: fight_group.params.unwrap_or_default(),
    }
}
