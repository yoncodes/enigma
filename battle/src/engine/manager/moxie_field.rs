use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct MoxieFields {
    amounts: HashMap<(i64, i64), i32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Credit {
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub amount: i32,
}

impl MoxieFields {
    pub fn amount(&self, owner_uid: i64, buff_uid: i64) -> Option<i32> {
        self.amounts.get(&(owner_uid, buff_uid)).copied()
    }

    pub fn credit(&mut self, update: Credit) {
        self.amounts
            .insert((update.owner_uid, update.buff_uid), update.amount);
    }

    pub fn remove(&mut self, owner_uid: i64, buff_uid: i64) {
        self.amounts.remove(&(owner_uid, buff_uid));
    }
}
