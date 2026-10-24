use super::PreferenceManager;
use crate::error::AppError;
use database::db::game::{items, simple_property};
use sonettobuf::{
    GetSimplePropertyReply, SetMainSceneSkinReply, SetSimplePropertyReply, SetUiStyleSkinReply,
    SimpleProperty, SimplePropertyPush,
};
use sqlx::SqlitePool;

const MAIN_SCENE_SKIN_PROPERTY_ID: i32 = 13;
const MAIN_UI_SKIN_PROPERTY_ID: i32 = 21;

impl PreferenceManager {
    pub async fn get_simple_property(
        &self,
        db: &SqlitePool,
    ) -> Result<GetSimplePropertyReply, AppError> {
        let simple_properties = simple_property::get_simple_properties(db, self.player_id).await?;

        Ok(GetSimplePropertyReply {
            simple_properties: simple_properties.into_iter().map(Into::into).collect(),
        })
    }

    pub async fn set_simple_property(
        &self,
        db: &SqlitePool,
        property_id: i32,
        property: String,
    ) -> Result<(SetSimplePropertyReply, SimplePropertyPush), AppError> {
        simple_property::set_simple_property(db, self.player_id, property_id, property.clone())
            .await?;

        Ok((
            SetSimplePropertyReply {},
            SimplePropertyPush {
                simple_property: Some(SimpleProperty {
                    id: Some(property_id),
                    property: Some(property),
                }),
            },
        ))
    }

    pub async fn set_main_scene_skin(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        item_id: i32,
    ) -> Result<SetMainSceneSkinReply, AppError> {
        let exists = tables.has_scene_skin(item_id);
        set_owned_skin_property(
            db,
            self.player_id,
            item_id,
            MAIN_SCENE_SKIN_PROPERTY_ID,
            exists,
        )
        .await?;
        Ok(SetMainSceneSkinReply {
            item_id: Some(item_id),
        })
    }

    pub async fn set_ui_style_skin(
        &self,
        db: &SqlitePool,
        tables: &config::GameDB,
        item_id: i32,
    ) -> Result<SetUiStyleSkinReply, AppError> {
        let exists = tables.has_ui_style_skin(item_id);
        set_owned_skin_property(
            db,
            self.player_id,
            item_id,
            MAIN_UI_SKIN_PROPERTY_ID,
            exists,
        )
        .await?;
        Ok(SetUiStyleSkinReply {
            item_id: Some(item_id),
        })
    }
}

async fn set_owned_skin_property(
    db: &SqlitePool,
    player_id: i64,
    item_id: i32,
    property_id: i32,
    exists: bool,
) -> Result<(), AppError> {
    if !exists
        || (item_id != 0
            && items::get_item(db, player_id, item_id as u32)
                .await?
                .is_none_or(|item| item.quantity <= 0))
    {
        return Err(AppError::InvalidRequest);
    }
    simple_property::set_simple_property(db, player_id, property_id, item_id.to_string()).await?;
    Ok(())
}

#[cfg(test)]
mod tests;
