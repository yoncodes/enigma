use super::*;

#[test]
fn values_match_lua_fight_enum() {
    assert_eq!(ClothSkillType::try_from(1), Ok(ClothSkillType::HeroUpgrade));
    assert_eq!(ClothSkillType::try_from(2), Ok(ClothSkillType::Rouge));
    assert_eq!(
        ClothSkillType::try_from(8),
        Ok(ClothSkillType::BattleSelection)
    );
    assert_eq!(ClothSkillType::try_from(9), Ok(ClothSkillType::ExtraRound));
    assert_eq!(
        ClothSkillType::try_from(10),
        Ok(ClothSkillType::TwinsSelect)
    );
}
