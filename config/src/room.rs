use crate::{GameDB, building_bonus::BuildingBonus, room_level::RoomLevel};

#[repr(i32)]
enum ManufactureConstant {
    InitialBuilding = 6,
}

impl GameDB {
    pub fn initial_room_rewards(&self) -> (Vec<i32>, Vec<i32>) {
        let packages = self
            .block_package
            .iter()
            .filter(|package| package.free)
            .map(|package| package.id)
            .collect();
        let buildings = self
            .manufacture_const
            .get(ManufactureConstant::InitialBuilding as i32)
            .and_then(|row| row.value.parse().ok())
            .filter(|id| self.room_building.get(*id).is_some())
            .into_iter()
            .collect();
        (packages, buildings)
    }

    pub fn room_level(&self, level: i32) -> Option<&RoomLevel> {
        self.room_level.iter().find(|row| row.level == level)
    }

    pub fn initial_room_level(&self) -> i32 {
        self.room_level
            .iter()
            .map(|row| row.level)
            .min()
            .unwrap_or_default()
    }

    pub fn building_bonus(&self, degree: i32) -> Option<&BuildingBonus> {
        self.building_bonus
            .iter()
            .filter(|row| row.build_degree <= degree)
            .max_by_key(|row| row.build_degree)
    }
}
