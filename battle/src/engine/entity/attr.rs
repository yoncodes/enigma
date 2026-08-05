use database::{db::game::equipment::Equipment, models::game::heros::HeroData};
use sonettobuf::HeroAttribute;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum AttrId {
    LostHp = 99,
    CurrentHp = 100,
    Hp = 101,
    Attack = 102,
    RealityDef = 103,
    MentalDef = 104,
    CriticalTechnique = 105,
    CriticalRate = 201,
    CriticalResistRate = 202,
    CriticalDmg = 203,
    CriticalDef = 204,
    DmgBonus = 205,
    DmgTakenReduction = 206,
    FinalDmgBonus = 207,
    FinalDmgReduction = 208,
    DmgHeal = 209,
    LeechRate = 210,
    UltimateMight = 211,
    HealingDone = 212,
    Penetration = 213,
    IncantationMight = 214,
    PlaymodeDmgIncrease = 215,
    PlaymodeDmgImmunity = 216,
    GenesisDmgBonus = 217,
    ReboundDmg = 218,
    ExtraDmg = 219,
    ReuseDmg = 220,
    UltimateMightMultiplier = 221,
    UltimateDmgBonus = 222,
    IncantationSkillUltMightMultiplier = 223,
    IncantationMightMultiplier = 224,
    ConduitMight = 231,
    PoisonDmgBonus = 301,
    PoisonDmgTakenReduction = 302,
    DazeResistance = 401,
    SleepResistance = 402,
    PetrifyResistance = 403,
    FreezeResistance = 404,
    DisarmResistance = 405,
    SilenceResistance = 406,
    SealResistance = 407,
    TaciturnResistance = 408,
    MoxieDownResistance = 409,
    StressUpResistance = 410,
    ControlResilience = 501,
    MoxieDownResilience = 502,
    HpBase = 601,
    AttackBase = 602,
    RealityDefBase = 603,
    MentalDefBase = 604,
    HpPercent = 605,
    AttackPercent = 606,
    RealityDefPercent = 607,
    MentalDefPercent = 608,
}

impl AttrId {
    pub const fn id(self) -> i32 {
        self as i32
    }

    pub fn from_raw(value: i32) -> Option<Self> {
        Some(match value {
            99 => Self::LostHp,
            100 => Self::CurrentHp,
            101 => Self::Hp,
            102 => Self::Attack,
            103 => Self::RealityDef,
            104 => Self::MentalDef,
            105 => Self::CriticalTechnique,
            201 => Self::CriticalRate,
            202 => Self::CriticalResistRate,
            203 => Self::CriticalDmg,
            204 => Self::CriticalDef,
            205 => Self::DmgBonus,
            206 => Self::DmgTakenReduction,
            207 => Self::FinalDmgBonus,
            208 => Self::FinalDmgReduction,
            209 => Self::DmgHeal,
            210 => Self::LeechRate,
            211 => Self::UltimateMight,
            212 => Self::HealingDone,
            213 => Self::Penetration,
            214 => Self::IncantationMight,
            215 => Self::PlaymodeDmgIncrease,
            216 => Self::PlaymodeDmgImmunity,
            217 => Self::GenesisDmgBonus,
            218 => Self::ReboundDmg,
            219 => Self::ExtraDmg,
            220 => Self::ReuseDmg,
            221 => Self::UltimateMightMultiplier,
            222 => Self::UltimateDmgBonus,
            223 => Self::IncantationSkillUltMightMultiplier,
            224 => Self::IncantationMightMultiplier,
            231 => Self::ConduitMight,
            301 => Self::PoisonDmgBonus,
            302 => Self::PoisonDmgTakenReduction,
            401 => Self::DazeResistance,
            402 => Self::SleepResistance,
            403 => Self::PetrifyResistance,
            404 => Self::FreezeResistance,
            405 => Self::DisarmResistance,
            406 => Self::SilenceResistance,
            407 => Self::SealResistance,
            408 => Self::TaciturnResistance,
            409 => Self::MoxieDownResistance,
            410 => Self::StressUpResistance,
            501 => Self::ControlResilience,
            502 => Self::MoxieDownResilience,
            601 => Self::HpBase,
            602 => Self::AttackBase,
            603 => Self::RealityDefBase,
            604 => Self::MentalDefBase,
            605 => Self::HpPercent,
            606 => Self::AttackPercent,
            607 => Self::RealityDefPercent,
            608 => Self::MentalDefPercent,
            _ => return None,
        })
    }
}

pub struct Attr;

impl Attr {
    pub fn get(hero: &HeroData, equips: &[Equipment]) -> HeroAttribute {
        super::stats::Stats::build_for_hero(hero, equips).base()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_attribute_ids_stop_at_the_config_boundary() {
        assert_eq!(AttrId::from_raw(102), Some(AttrId::Attack));
        assert_eq!(AttrId::from_raw(211), Some(AttrId::UltimateMight));
        assert_eq!(AttrId::from_raw(231), Some(AttrId::ConduitMight));
        assert_eq!(AttrId::from_raw(608), Some(AttrId::MentalDefPercent));
        assert_eq!(AttrId::from_raw(999), None);
    }
}
