use crate::engine::skill::rule::ConfiguredRuleKey;

use super::RoundDeterminism;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomBuffChoice {
    pub rule: ConfiguredRuleKey,
    pub source_uid: i64,
    pub target_uid: i64,
    pub buff_ids: Vec<i32>,
}

impl RoundDeterminism {
    pub fn replace_random_buff_pools(
        &mut self,
        choices: impl IntoIterator<Item = RandomBuffChoice>,
    ) {
        self.random_buff_pools = choices.into_iter().collect();
    }

    pub fn take_random_buff_pool(
        &mut self,
        rule: ConfiguredRuleKey,
        source_uid: i64,
        target_uid: i64,
        count: usize,
        candidates: &[i32],
    ) -> Option<Vec<i32>> {
        let index = self.random_buff_pools.iter().position(|choice| {
            choice.rule == rule
                && choice.source_uid == source_uid
                && choice.target_uid == target_uid
        })?;
        let choice = self.random_buff_pools.remove(index)?;
        (choice.buff_ids.len() == count
            && choice.buff_ids.iter().enumerate().all(|(index, id)| {
                candidates.contains(id) && !choice.buff_ids[..index].contains(id)
            }))
        .then_some(choice.buff_ids)
    }
}
