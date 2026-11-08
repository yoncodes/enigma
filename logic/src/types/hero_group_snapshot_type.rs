#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum HeroGroupSnapshotType {
    Common = 2,
    Resources = 3,
    Season = 5,
    Season123 = 6,
    Season123Retail = 7,
    Season166Base = 8,
    Season166Train = 9,
    TowerPermanentAndLimit = 10,
    TowerBoss = 11,
    Act183Normal = 12,
    Act183Boss = 13,
    Shelter = 14,
    Survival = 15,
    FiveHero = 16,
    TowerComposeNormal = 17,
    TowerComposeBoss = 18,
    AbyssPreset = 19,
    Rouge2 = 20,
    AtomicDungeon = 21,
    BossRushActMode = 22,
    Abyss = 23,
}

impl HeroGroupSnapshotType {
    pub const ALL_DESCENDING: [Self; 21] = [
        Self::Abyss,
        Self::BossRushActMode,
        Self::AtomicDungeon,
        Self::Rouge2,
        Self::AbyssPreset,
        Self::TowerComposeBoss,
        Self::TowerComposeNormal,
        Self::FiveHero,
        Self::Survival,
        Self::Shelter,
        Self::Act183Boss,
        Self::Act183Normal,
        Self::TowerBoss,
        Self::TowerPermanentAndLimit,
        Self::Season166Train,
        Self::Season166Base,
        Self::Season123Retail,
        Self::Season123,
        Self::Season,
        Self::Resources,
        Self::Common,
    ];

    pub const fn id(self) -> i32 {
        self as i32
    }

    pub fn from_id(id: i32) -> Option<Self> {
        Self::ALL_DESCENDING
            .into_iter()
            .find(|snapshot_type| snapshot_type.id() == id)
    }
}
