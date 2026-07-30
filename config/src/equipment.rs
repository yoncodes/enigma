use crate::{GameDB, equip_break_cost::EquipBreakCost, equip_strengthen_cost::EquipStrengthenCost};

impl GameDB {
    pub fn equip_break_cost(&self, rare: i32, break_level: i32) -> Option<&EquipBreakCost> {
        self.equip_break_cost
            .iter()
            .find(|row| row.rare == rare && row.break_level == break_level)
    }

    pub fn equip_strengthen_cost(&self, rare: i32, level: i32) -> Option<&EquipStrengthenCost> {
        self.equip_strengthen_cost
            .iter()
            .find(|row| row.rare == rare && row.level == level)
    }

    pub fn max_equip_progression(&self, rare: i32) -> Option<&EquipBreakCost> {
        self.equip_break_cost
            .iter()
            .filter(|row| row.rare == rare)
            .max_by_key(|row| row.break_level)
    }

    pub fn equip_universal_refine_id(&self) -> Option<i32> {
        self.equip_const.get(14)?.value.parse().ok()
    }

    pub fn equip_max_refine_level(&self) -> Option<i32> {
        self.equip_const.get(15)?.value.parse().ok()
    }

    pub fn equip_refine_rarity_threshold(&self) -> Option<i32> {
        self.equip_const.get(16)?.value.parse().ok()
    }

    pub fn is_normal_equipment(&self, equip: &crate::equip::Equip) -> bool {
        equip.is_exp_equip == 0
            && equip.is_sp_refine == 0
            && self
                .equip_universal_refine_id()
                .is_some_and(|universal_id| equip.id != universal_id)
    }
}
