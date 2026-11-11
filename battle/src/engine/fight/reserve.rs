#[derive(Debug, Clone, PartialEq)]
pub struct Promotion {
    pub defeated_uid: i64,
    pub entering_uid: i64,
    pub position: i32,
    pub team_type: i32,
    pub entering: sonettobuf::FightEntityInfo,
    pub cycle: Option<CycleSubstitution>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CycleSubstitution {
    pub removed_uid: i64,
    pub reserve: sonettobuf::FightEntityInfo,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PromotionBatch {
    promotions: Vec<Promotion>,
    completion: PromotionBatchCompletion,
}

impl PromotionBatch {
    pub fn new(promotions: Vec<Promotion>) -> Self {
        let completion = PromotionBatchCompletion {
            cycles: promotions
                .iter()
                .filter_map(|promotion| promotion.cycle.clone())
                .collect(),
        };
        Self {
            promotions,
            completion,
        }
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

    pub fn completion(&self) -> &PromotionBatchCompletion {
        &self.completion
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PromotionBatchCompletion {
    pub cycles: Vec<CycleSubstitution>,
}

impl PromotionBatchCompletion {
    pub fn is_empty(&self) -> bool {
        self.cycles.is_empty()
    }
}
