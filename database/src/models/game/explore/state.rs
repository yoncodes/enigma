use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MapState {
    pub map_id: i32,
    pub posx: i32,
    pub posy: i32,
    pub dir: i32,
    pub challenge_count: i32,
    pub interacts: Vec<InteractState>,
    pub area_ids: Vec<i32>,
    pub move_nodes: String,
    #[serde(default)]
    pub items: Vec<ItemState>,
    #[serde(default)]
    pub use_item_uid: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemState {
    pub uid: i64,
    pub item_id: i32,
    pub quantity: i32,
    pub status: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractState {
    pub id: i32,
    pub kind: i32,
    pub status: i32,
    pub status2: String,
    pub posx: i32,
    pub posy: i32,
    pub dir: i32,
}
