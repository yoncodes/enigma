use sonettobuf::CardInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum UniversalCardSkill {
    RankOne = 30_000_001,
    RankTwo = 30_000_002,
}

impl UniversalCardSkill {
    pub fn from_rank(rank: i32) -> Option<Self> {
        match rank {
            1 => Some(Self::RankOne),
            2 => Some(Self::RankTwo),
            _ => None,
        }
    }

    pub const fn id(self) -> i32 {
        self as i32
    }
}

impl TryFrom<i32> for UniversalCardSkill {
    type Error = ();

    fn try_from(skill_id: i32) -> Result<Self, Self::Error> {
        match skill_id {
            value if value == Self::RankOne.id() => Ok(Self::RankOne),
            value if value == Self::RankTwo.id() => Ok(Self::RankTwo),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayedCard {
    pub card: CardInfo,
    pub caster_uid: i64,
    pub card_index: i32,
    pub hand_index: Option<usize>,
    pub skill_id: i32,
    pub rank_change_pending: bool,
    pub rewritten: bool,
    pub target_uid: Option<i64>,
    pub recorded_skill: Option<crate::engine::skill::action::SkillRequest>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardPlayChoice {
    pub source: CardInfo,
    pub played: CardInfo,
}

#[cfg(test)]
mod tests {
    use super::UniversalCardSkill;

    #[test]
    fn universal_card_protocol_ids_map_to_their_exact_ranks() {
        assert_eq!(
            UniversalCardSkill::from_rank(1),
            Some(UniversalCardSkill::RankOne)
        );
        assert_eq!(
            UniversalCardSkill::from_rank(2),
            Some(UniversalCardSkill::RankTwo)
        );
        assert_eq!(UniversalCardSkill::from_rank(3), None);
        assert_eq!(
            UniversalCardSkill::try_from(30_000_001),
            Ok(UniversalCardSkill::RankOne)
        );
        assert!(UniversalCardSkill::try_from(30_000_003).is_err());
    }
}
