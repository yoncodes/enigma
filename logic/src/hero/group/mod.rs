use super::HeroManager;
use crate::{error::AppError, types::hero_group_snapshot_type::HeroGroupSnapshotType};
use database::{
    db::game::{equipment, hero_group_snapshots, hero_groups},
    models::game::hero_groups as model,
};
use sonettobuf::{
    ChangeHeroGroupSelectReply, CheckHeroGroupNameReply, DeleteHeroGroupReply,
    GetHeroGroupCommonListReply, GetHeroGroupListReply, HeroGroupEquip, SetHeroGroupEquipReply,
    UpdateHeroGroupNameReply, UpdateHeroGroupReply, UpdateHeroGroupSortReply,
};
use sqlx::SqlitePool;
use std::collections::HashSet;

mod snapshot;

const HERO_GROUP_NAME_LIMIT_ID: i32 = 141;

impl HeroManager {
    pub fn check_group_name(&self, name: &str) -> Result<CheckHeroGroupNameReply, AppError> {
        let max_len = config::configs::get()
            .r#const
            .get(HERO_GROUP_NAME_LIMIT_ID)
            .and_then(|row| row.value.parse::<usize>().ok())
            .ok_or(AppError::InvalidRequest)?;

        if name.trim().is_empty() || name.chars().count() > max_len {
            return Err(AppError::InvalidRequest);
        }

        Ok(CheckHeroGroupNameReply {})
    }

    pub async fn update_group_name(
        &self,
        db: &SqlitePool,
        snapshot_id: i32,
        snapshot_sub_id: i32,
        name: String,
    ) -> Result<UpdateHeroGroupNameReply, AppError> {
        self.check_group_name(&name)?;
        if snapshot_id <= 0 || snapshot_sub_id <= 0 {
            return Err(AppError::InvalidRequest);
        }

        let changed = if snapshot_id == HeroGroupSnapshotType::Common.id() {
            hero_group_snapshots::rename_common_group_snapshot(
                db,
                self.player_id,
                snapshot_id,
                snapshot_sub_id,
                &name,
            )
            .await?
        } else {
            hero_group_snapshots::rename_hero_group_snapshot(
                db,
                self.player_id,
                snapshot_id,
                snapshot_sub_id,
                &name,
            )
            .await?
        };

        if !changed {
            return Err(AppError::InvalidRequest);
        }

        Ok(UpdateHeroGroupNameReply {
            id: Some(snapshot_id),
            current_select: Some(snapshot_sub_id),
            name: Some(name),
        })
    }

    pub async fn update_group_sort(
        &self,
        db: &SqlitePool,
        snapshot_id: i32,
        sort_sub_ids: Vec<i32>,
    ) -> Result<UpdateHeroGroupSortReply, AppError> {
        if snapshot_id <= 0
            || sort_sub_ids.is_empty()
            || sort_sub_ids.iter().any(|id| *id <= 0)
            || !hero_group_snapshots::replace_hero_group_sort(
                db,
                self.player_id,
                snapshot_id,
                &sort_sub_ids,
                snapshot_id == HeroGroupSnapshotType::Common.id(),
            )
            .await?
        {
            return Err(AppError::InvalidRequest);
        }

        Ok(UpdateHeroGroupSortReply {
            snapshot_id: Some(snapshot_id),
            sort_sub_ids,
        })
    }

    pub async fn delete_group(
        &self,
        db: &SqlitePool,
        snapshot_id: i32,
        snapshot_sub_id: i32,
    ) -> Result<DeleteHeroGroupReply, AppError> {
        if snapshot_id <= 0 || snapshot_sub_id <= 0 {
            return Err(AppError::InvalidRequest);
        }
        let sort_sub_ids = hero_group_snapshots::delete_hero_group(
            db,
            self.player_id,
            snapshot_id,
            snapshot_sub_id,
            snapshot_id == HeroGroupSnapshotType::Common.id(),
        )
        .await?
        .ok_or(AppError::InvalidRequest)?;

        Ok(DeleteHeroGroupReply {
            snapshot_id: Some(snapshot_id),
            snapshot_sub_id: Some(snapshot_sub_id),
            sort_sub_ids,
        })
    }

    pub async fn update_group(
        &self,
        db: &SqlitePool,
        group: sonettobuf::HeroGroupInfo,
    ) -> Result<UpdateHeroGroupReply, AppError> {
        let name = group.name.unwrap_or_default();
        if !name.is_empty() {
            self.check_group_name(&name)?;
        }
        let hero_uids = group
            .hero_list
            .iter()
            .copied()
            .filter(|uid| *uid != 0)
            .collect::<HashSet<_>>();
        let valid_activity_equips = |equips: &[HeroGroupEquip]| {
            equips.iter().all(|equip| equip.index.unwrap_or(-1) >= 0)
                && equips
                    .iter()
                    .filter_map(|equip| equip.index)
                    .collect::<HashSet<_>>()
                    .len()
                    == equips.len()
        };
        if group.group_id <= 0
            || hero_uids.len() != group.hero_list.iter().filter(|uid| **uid != 0).count()
            || !valid_activity_equips(&group.activity104_equips)
        {
            return Err(AppError::InvalidRequest);
        }
        self.validate_group_equips(db, &group.hero_list, &group.equips)
            .await?;

        let saved = model::HeroGroupInfo {
            group_id: group.group_id,
            hero_list: group.hero_list,
            name,
            cloth_id: group.cloth_id.unwrap_or(1),
            equips: group
                .equips
                .into_iter()
                .map(|equip| model::HeroGroupEquip {
                    index: equip.index.unwrap_or_default(),
                    equip_uids: equip.equip_uid,
                })
                .collect(),
            activity104_equips: group
                .activity104_equips
                .into_iter()
                .map(|equip| model::HeroGroupEquip {
                    index: equip.index.unwrap_or_default(),
                    equip_uids: equip.equip_uid,
                })
                .collect(),
            assist_boss_id: group.assist_boss_id.unwrap_or_default(),
            params: group.params.unwrap_or_default(),
        };
        if !hero_groups::update_hero_group(db, self.player_id, &saved).await? {
            return Err(AppError::InvalidRequest);
        }

        Ok(UpdateHeroGroupReply {
            group_info: Some(saved.into()),
        })
    }

    pub async fn group_common_list(
        &self,
        db: &SqlitePool,
    ) -> Result<GetHeroGroupCommonListReply, AppError> {
        Ok(GetHeroGroupCommonListReply {
            hero_group_commons: hero_groups::get_hero_groups_common(db, self.player_id)
                .await?
                .into_iter()
                .map(Into::into)
                .collect(),
            hero_gourp_types: hero_groups::get_hero_group_types(db, self.player_id)
                .await?
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }

    pub async fn group_list(&self, db: &SqlitePool) -> Result<GetHeroGroupListReply, AppError> {
        Ok(GetHeroGroupListReply {
            group_info_list: hero_groups::get_current_hero_group(db, self.player_id)
                .await?
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }

    pub async fn change_group_selection(
        &self,
        db: &SqlitePool,
        type_id: i32,
        current_select: i32,
    ) -> Result<ChangeHeroGroupSelectReply, AppError> {
        if type_id <= 0
            || current_select < 0
            || (current_select > 0
                && hero_groups::get_hero_group(db, self.player_id, current_select)
                    .await?
                    .is_none())
        {
            return Err(AppError::InvalidRequest);
        }

        hero_groups::set_current_selection(db, self.player_id, type_id, current_select).await?;

        Ok(ChangeHeroGroupSelectReply {
            id: Some(type_id),
            current_select: Some(current_select),
        })
    }

    pub async fn set_group_equip(
        &self,
        db: &SqlitePool,
        group_id: i32,
        equip: HeroGroupEquip,
    ) -> Result<SetHeroGroupEquipReply, AppError> {
        let index = equip.index.ok_or(AppError::InvalidRequest)?;
        let equip_uids = equip.equip_uid;
        let group = hero_groups::get_hero_group(db, self.player_id, group_id)
            .await?
            .ok_or(AppError::InvalidRequest)?;
        let mut assignments = group
            .equips
            .into_iter()
            .filter(|assigned| assigned.index != index)
            .map(Into::into)
            .collect::<Vec<HeroGroupEquip>>();
        assignments.push(HeroGroupEquip {
            index: Some(index),
            equip_uid: equip_uids.clone(),
        });
        self.validate_group_equips(db, &group.hero_list, &assignments)
            .await?;

        hero_groups::set_hero_group_equip(db, self.player_id, group_id, index, equip_uids.clone())
            .await?;

        Ok(SetHeroGroupEquipReply {
            group_id: Some(group_id),
            equip: Some(HeroGroupEquip {
                index: Some(index),
                equip_uid: equip_uids,
            }),
        })
    }

    async fn validate_group_equips(
        &self,
        db: &SqlitePool,
        hero_list: &[i64],
        equips: &[HeroGroupEquip],
    ) -> Result<(), AppError> {
        let tables = config::configs::get();
        let universal_id = tables
            .equip_universal_refine_id()
            .ok_or(AppError::InvalidRequest)?;
        let mut indexes = HashSet::with_capacity(equips.len());
        let mut assigned_uids = HashSet::with_capacity(equips.len());

        for equip in equips {
            let index = equip.index.ok_or(AppError::InvalidRequest)?;
            let slot = usize::try_from(index).map_err(|_| AppError::InvalidRequest)?;
            if !indexes.insert(index) || equip.equip_uid.len() != 1 || slot >= hero_list.len() {
                return Err(AppError::InvalidRequest);
            }

            let equip_uid = equip.equip_uid[0];
            if equip_uid == 0 {
                continue;
            }
            if hero_list[slot] == 0 || !assigned_uids.insert(equip_uid) {
                return Err(AppError::InvalidRequest);
            }
            let owned = equipment::get_equipment_by_uid(db, self.player_id, equip_uid)
                .await
                .map_err(|_| AppError::InvalidRequest)?;
            let equip_config = tables
                .equip
                .get(owned.equip_id)
                .ok_or(AppError::InvalidRequest)?;
            if equip_config.is_exp_equip == 1 || owned.equip_id == universal_id {
                return Err(AppError::InvalidRequest);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod test;
