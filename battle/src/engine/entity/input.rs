#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeroBuildInput {
    pub uid: i64,
    pub user_id: i64,
    pub hero_id: i32,
    pub skin: i32,
    pub level: i32,
    pub rank: i32,
    pub ex_skill_level: i32,
    pub talent: i32,
    pub talent_style: i32,
    pub talent_placements: Vec<i32>,
    pub destiny_rank: i32,
    pub destiny_stone: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EquipmentBuildInput {
    pub uid: i64,
    pub equip_id: i32,
    pub level: i32,
    pub break_level: i32,
    pub refine_level: i32,
}
