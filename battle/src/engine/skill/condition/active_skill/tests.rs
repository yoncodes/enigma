use super::*;

#[test]
fn specific_skill_preserves_group_and_rank_selectors() {
    assert_eq!(
        specific_skill(66203, "UseSpecificSkill", &["1".into(), "0".into()]),
        Some(ParsedConditionKind::SpecificSkill { group: 1, rank: 0 })
    );
    assert_eq!(
        specific_skill(66210, "UseSpecificSkill", &["4".into()]),
        Some(ParsedConditionKind::SpecificSkill { group: 4, rank: 0 })
    );
    assert_eq!(
        specific_skill(66203, "UseSpecificSkill", &["5".into(), "0".into()]),
        Some(ParsedConditionKind::SpecificSkill { group: 5, rank: 0 })
    );
    assert!(specific_skill(66210, "UseSpecificSkill", &["0".into()]).is_none());
    assert!(
        specific_skill(
            66210,
            "UseSpecificSkill",
            &["4".into(), "0".into(), "999".into()]
        )
        .is_none()
    );
    assert!(specific_skill(66210, "UseSpecificSkill", &["6".into(), "1".into()]).is_none());
}

#[test]
fn can_use_skill_keeps_its_exact_semantic_kind() {
    assert_eq!(
        can_use_skill(615201, "CanUseSkill", &["308801711".into()]),
        Some(ParsedConditionKind::CanUseSkill(vec![308801711]))
    );
}

#[test]
fn current_skill_level_preserves_all_configured_ranks() {
    assert_eq!(
        rank(620402, "CurrSkillLevel", &["2,3".into()]),
        Some(ParsedConditionKind::ActiveSkillRank {
            compare: ConditionCompare::Equal,
            ranks: vec![2, 3],
        })
    );
}

#[test]
fn aleph_active_attack_conditions_use_the_ally_action_lane() {
    assert_eq!(
        active_ally_use(502212, "ActiveUseSkill", &["0".into()]),
        Some(ParsedConditionKind::ActiveAllyUseSkill { slot: 0 })
    );
    assert_eq!(
        hurt_skill(501212, "UseHurtSkill", &[]),
        Some(ParsedConditionKind::UseHurtSkill)
    );
}
