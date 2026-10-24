use super::claims::coalesced_material_changes;
use crate::reward;

#[test]
fn double_bonus_materials_are_aggregated_by_type_and_id() {
    let mut rewards = reward::parse("2#2#300|1#481006#3");
    rewards.extend(reward::parse("2#2#60|1#481006#2"));

    assert_eq!(
        coalesced_material_changes(&rewards),
        vec![(1, 481006, 5), (2, 2, 360)]
    );
}
