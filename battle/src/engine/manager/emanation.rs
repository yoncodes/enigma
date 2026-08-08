use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmanationKind {
    Blue,
    Purple,
    Green,
}

impl EmanationKind {
    pub const fn from_id(id: i32) -> Option<Self> {
        match id {
            1 => Some(Self::Blue),
            2 => Some(Self::Purple),
            3 => Some(Self::Green),
            _ => None,
        }
    }

    const fn index(self) -> usize {
        match self {
            Self::Blue => 0,
            Self::Purple => 1,
            Self::Green => 2,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct EmanationManager {
    // Semantic order used by config groups: Blue, Purple, Green.
    counts: HashMap<i64, [i32; 3]>,
}

impl EmanationManager {
    pub fn select(&mut self, owner_uid: i64, packed: i32) -> bool {
        let counts = [packed / 100, packed / 10 % 10, packed % 10];
        if owner_uid == 0 || counts.iter().any(|count| *count < 0) {
            return false;
        }
        self.counts.insert(owner_uid, counts);
        true
    }

    pub fn choose(&self, owner_uid: i64, roll: i32) -> Option<usize> {
        let counts = self.counts.get(&owner_uid)?;
        let total = counts.iter().sum::<i32>();
        if total <= 0 {
            return None;
        }
        let mut point = roll.clamp(0, 999) * total / 1000;
        for (index, count) in counts.iter().enumerate() {
            if point < *count {
                return Some(index);
            }
            point -= *count;
        }
        None
    }

    pub fn counts(&self, owner_uid: i64) -> [i32; 3] {
        self.counts.get(&owner_uid).copied().unwrap_or_default()
    }

    pub fn count(&self, owner_uid: i64, kind: EmanationKind) -> i32 {
        self.counts(owner_uid)[kind.index()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_is_owned_per_entity() {
        let mut manager = EmanationManager::default();
        assert!(manager.select(10, 110));
        assert_eq!(manager.counts(10), [1, 1, 0]);
        assert_eq!(manager.count(10, EmanationKind::Blue), 1);
        assert_eq!(manager.count(10, EmanationKind::Purple), 1);
        assert_eq!(manager.count(10, EmanationKind::Green), 0);
        assert_eq!(manager.choose(10, 0), Some(0));
        assert_eq!(manager.choose(10, 999), Some(1));
        assert_eq!(manager.counts(20), [0, 0, 0]);
    }
}
