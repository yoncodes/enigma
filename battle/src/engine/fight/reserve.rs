use sonettobuf::FightEntityInfo;

#[derive(Debug, Clone, PartialEq)]
pub struct Promotion {
    pub defeated_uid: i64,
    pub entering_uid: i64,
    pub position: i32,
    pub team_type: i32,
    pub entering: FightEntityInfo,
    pub queued_reserve: Option<ReserveQueued>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CycleCommand {
    pub defeated_uid: i64,
    pub team_type: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReserveQueued {
    pub team_type: i32,
    pub entity: FightEntityInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReserveRetired {
    pub uid: i64,
    pub team_type: i32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PromotionBatch {
    promotions: Vec<Promotion>,
}

impl PromotionBatch {
    pub fn new(promotions: Vec<Promotion>) -> Self {
        Self { promotions }
    }

    pub fn is_empty(&self) -> bool {
        self.promotions.is_empty()
    }

    pub fn len(&self) -> usize {
        self.promotions.len()
    }

    pub fn promotions(&self) -> &[Promotion] {
        &self.promotions
    }
}
