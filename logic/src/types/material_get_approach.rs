#[allow(dead_code)]
#[derive(Clone, Copy)]
pub enum MaterialGetApproach {
    ItemUseReward = 3,
    Mail = 4,
    Task = 8,
    TaskAct = 13,
    StorePurchase = 15,
    SignIn = 20,
    Activity = 25,
    DungeonRewardPoint = 28,
    Charge = 31,
    RoomProductLine = 34,
    RoomProductChange = 38,
    MonthCard = 42,
    Explore = 45,
    RoomGainFaith = 46,
    RoomInteraction = 47,
    BattlePass = 49,
    NoviceStageReward = 54,
    AstrologyStarReward = 62,
    Act1_6SkillLvDown = 84,
    Act1_6SkillReset = 85,
    V1a8Act157ComponentReward = 96,
    V2a2Act169SummonNewPick = 119,
    Tower = 124,
    SmallMonthCard = 125,
    AutoChessPveReward = 127,
    SeasonCard = 129,
    LifeCircleSign = 133,
    AutoChessRankReward = 134,
    Activity197View = 138,
    SkinCoupon = 139,
    Act128MilestoneBonus = 152,
    Birthday = 153,
    ArcadeReward = 154,
    PartyClothSummon = 158,
    CommandStationPaperReward = 169,
    Act236Reward = 171,
    ActBp = 173,
    Act239Bonus = 177,
}

impl MaterialGetApproach {
    pub const fn id(self) -> u32 {
        self as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn act_bp_uses_its_exact_client_approach() {
        assert_eq!(MaterialGetApproach::ActBp.id(), 173);
    }

    #[test]
    fn act128_milestone_uses_its_captured_approach() {
        assert_eq!(MaterialGetApproach::Act128MilestoneBonus.id(), 152);
    }

    #[test]
    fn act236_reward_uses_its_captured_approach() {
        assert_eq!(MaterialGetApproach::Act236Reward.id(), 171);
    }

    #[test]
    fn act239_bonus_uses_its_captured_approach() {
        assert_eq!(MaterialGetApproach::Act239Bonus.id(), 177);
    }

    #[test]
    fn arcade_reward_uses_its_captured_approach() {
        assert_eq!(MaterialGetApproach::ArcadeReward.id(), 154);
    }
}
