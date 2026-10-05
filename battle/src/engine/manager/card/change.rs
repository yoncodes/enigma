use sonettobuf::CardInfo;

#[derive(Debug, Clone, PartialEq)]
pub enum CardChange {
    DeckCount {
        deck_num: i32,
        team_type: i32,
    },
    CardsPush {
        cards: Vec<CardInfo>,
        team_type: i32,
    },
    AddHand {
        target_uid: i64,
        card: CardInfo,
    },
    OwnedAddHand {
        card: CardInfo,
        team_type: i32,
    },
    SpCardAdd {
        target_uid: i64,
        skill_id: i32,
        reserve_id: i64,
        team_type: i32,
    },
    ChangeToTemp {
        target_uid: i64,
        reserve_str: String,
        team_type: i32,
    },
    Enchant {
        cards: Vec<CardInfo>,
        indices: Vec<usize>,
        team_type: i32,
    },
    MarkTemporary {
        indices: Vec<usize>,
        team_type: i32,
        config_effect: i32,
    },
    CardsCompose {
        cards: Vec<CardInfo>,
        team_type: i32,
    },
}
