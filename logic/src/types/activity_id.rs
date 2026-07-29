#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[repr(i32)]
pub enum ActivityId {
    StoryShow = 10010,
    DreamShow = 10011,
    ClassShow = 10012,
    WeekWalkDeepShow = 10013,
    WeekWalkHeartShow = 10015,
    GiftOfTheBeginning = 11924,
    StoryDeduction = 12104,
    Tower = 12320,
    V2a9EnterView130501 = 130501,
    V2a9Dungeon130502 = 130502,
    V2a9DungeonStore130503 = 130503,
    V2a9Outside = 130504,
    V2a9BossRush130505 = 130505,
    V2a9EnterView2 = 130506,
    V2a9Dungeon2 = 130507,
    V2a9ReactivityStore130508 = 130508,
    SilverLitNight = 13108,
    ManyFacesOfParis = 13119,
    MoonlightGardening = 13506,
    V3a6Abyss = 13601,
    V3a6DoubleDrop = 13602,
    V3a6EnterView = 13603,
    V3a6Dungeon = 13604,
    V3a6DungeonStore = 13605,
    V3a6YaMi = 13608,
    V3a6BossRush = 13609,
    V3a6CultivationDestiny = 13610,
}

impl ActivityId {
    pub const fn id(self) -> i32 {
        self as i32
    }
}
