use sonettobuf::{
    SurvivalHandbookBox, SurvivalModBox, SurvivalOutSideGetInfoReply, SurvivalOutSideInfo,
    SurvivalOutSideMall, SurvivalOutSideMallItem, SurvivalOutSideTechBox, SurvivalRole,
    SurvivalRoleBox,
};

pub fn outside_info(tables: &config::GameDB) -> SurvivalOutSideGetInfoReply {
    let default_mod = tables
        .survival_hardness_mod
        .iter()
        .find(|row| row.optional == 0 && row.sub_tab == 0)
        .map(|row| row.id)
        .into_iter()
        .collect::<Vec<_>>();
    let mut roles = tables
        .survival_role
        .iter()
        .filter(|row| row.isonline != 0)
        .collect::<Vec<_>>();
    roles.sort_by_key(|row| (row.disposition_type, row.tech_sprite_id, row.id));

    SurvivalOutSideGetInfoReply {
        info: Some(SurvivalOutSideInfo {
            season: tables
                .activity
                .iter()
                .filter(|row| row.type_id == 200)
                .map(|row| row.id)
                .max(),
            score: Some(0),
            in_week: Some(false),
            client_data: Some(String::new()),
            handbook_box: Some(SurvivalHandbookBox::default()),
            out_side_tech_box: Some(SurvivalOutSideTechBox::default()),
            role_box: Some(SurvivalRoleBox {
                roles: roles
                    .into_iter()
                    .map(|row| SurvivalRole {
                        role_id: Some(row.id),
                        progress: Some(0),
                        max_progress: Some(role_max_progress(&row.conditions)),
                        unlocked: Some(row.conditions.is_empty()),
                        is_new: Some(false),
                    })
                    .collect(),
            }),
            mod_box: Some(SurvivalModBox {
                unlock_id: default_mod.clone(),
                new_ids: default_mod,
            }),
            mall: Some(SurvivalOutSideMall {
                items: tables
                    .survival_reward_shop
                    .iter()
                    .map(|row| SurvivalOutSideMallItem {
                        id: Some(row.id),
                        count: Some(row.max_buy_count),
                    })
                    .collect(),
            }),
            ..Default::default()
        }),
    }
}

fn role_max_progress(condition: &str) -> i32 {
    condition
        .split('#')
        .nth(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
