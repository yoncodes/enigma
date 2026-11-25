use super::*;
use crate::engine::skill::effect::SkillEffectCatalog;

#[test]
fn only_the_channel_psychube_self_grant_runs_in_its_published_phase() {
    crate::test_support::init_config();
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    for skill_id in 435011..=435015 {
        let slot = &catalog.get(skill_id).unwrap().slots[0];
        assert_eq!(
            resolve(PassiveSourceKind::Psychube, Some(slot)),
            ReactionTiming::Immediate
        );
    }
    let channel = &catalog.get(435011).unwrap().slots[0];
    assert_eq!(
        resolve(PassiveSourceKind::BattleRule, Some(channel)),
        ReactionTiming::AfterSkill
    );
    for skill_id in [431111, 435911, 91110110] {
        let slot = &catalog.get(skill_id).unwrap().slots[0];
        assert_eq!(
            resolve(PassiveSourceKind::Psychube, Some(slot)),
            ReactionTiming::AfterSkill
        );
    }
}
