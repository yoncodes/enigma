#[test]
fn destiny_stones_replace_base_battle_tags() {
    crate::init_test_config();
    let tables = crate::configs::get();
    let groups = tables.activity128_bond_groups(13919).unwrap();
    let damage = groups.iter().find(|group| group.group_id == 10019).unwrap();
    let support = groups.iter().find(|group| group.group_id == 10020).unwrap();
    let base = tables.activity128_bond_hero(3051, 0).unwrap();
    let destiny = tables.activity128_bond_hero(3051, 305101).unwrap();

    assert_eq!(base.battle_tags, vec![101]);
    assert_eq!(destiny.battle_tags, vec![102, 106, 116]);
    assert!(damage.matches(&base));
    assert!(!support.matches(&base));
    assert!(damage.matches(&destiny));
    assert!(support.matches(&destiny));
}
