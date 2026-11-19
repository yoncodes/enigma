pub(super) const ATTACKER_BUFF_UID_START: i64 = 2;
pub(super) const DEFENDER_BUFF_UID_START: i64 = 100001;
const ATTACKER_BUFF_UID_START_V7: i64 = 1002;
const NORMAL_UID_STEP: i64 = 2;

pub(super) fn attacker_start(fight_version: i32) -> i64 {
    match fight_version {
        7 => ATTACKER_BUFF_UID_START_V7,
        _ => ATTACKER_BUFF_UID_START,
    }
}

pub(super) fn uses_shared_lane(fight_version: i32) -> bool {
    fight_version == 7
}

#[derive(Debug, Clone)]
pub(super) struct BuffUidAllocator {
    first: i64,
    last: i64,
    allocated: bool,
    last_lane: Option<UidLane>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UidLane {
    Normal,
    Child,
    Observed,
}

impl BuffUidAllocator {
    pub(super) fn new(first: i64) -> Self {
        Self {
            first,
            last: first - NORMAL_UID_STEP,
            allocated: false,
            last_lane: None,
        }
    }

    pub(super) fn next(&mut self) -> i64 {
        self.last = if self.allocated {
            self.last + NORMAL_UID_STEP
        } else {
            self.first
        };
        self.allocated = true;
        self.last_lane = Some(UidLane::Normal);
        self.last
    }

    pub(super) fn next_child(&mut self) -> i64 {
        self.last = if self.allocated {
            self.last + 1
        } else {
            self.first
        };
        self.allocated = true;
        self.last_lane = Some(UidLane::Child);
        self.last
    }

    pub(super) fn observe(&mut self, uid: i64) {
        if uid > self.last {
            self.last = uid;
        }
        self.allocated = true;
        self.last_lane = Some(UidLane::Observed);
    }

    pub(super) fn has_reached(&self, uid: i64) -> bool {
        self.allocated && self.last >= uid
    }

    pub(super) fn last_was_child(&self) -> bool {
        self.last_lane == Some(UidLane::Child)
    }

    pub(super) fn last_child(&self) -> Option<i64> {
        self.last_was_child().then_some(self.last)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attacker_lane_is_selected_by_the_exact_fight_protocol_version() {
        assert_eq!(BuffUidAllocator::new(attacker_start(6)).next(), 2);
        assert_eq!(BuffUidAllocator::new(attacker_start(7)).next(), 1002);
        assert!(!uses_shared_lane(6));
        assert!(uses_shared_lane(7));
        assert_eq!(
            BuffUidAllocator::new(DEFENDER_BUFF_UID_START).next(),
            100001
        );
    }
}
