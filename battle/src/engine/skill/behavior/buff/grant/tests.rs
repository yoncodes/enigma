use crate::catalog::BattleCatalog;

use super::*;

#[test]
fn normalizes_random_buff_pools_in_order() {
    crate::test_support::init_config();
    let pool = |id| ParsedBehavior::new(20022, "AddBuffRanTypeId", vec![id, 1]);
    assert_eq!(
        random_buff_pool(&pool(30830151)),
        Some(vec![308301511, 308301512, 308301513, 308301514])
    );
    assert!(random_buff_pool(&pool(-1)).is_none());
}

#[test]
fn random_pools_validate_the_whole_entry_before_selecting_a_route() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::try_global().unwrap();
    assert!(pool_entries(catalog, "30920153,4#30920155,bad").is_none());
    assert!(pool_entries(catalog, "30920153,4#30920155,4,2").is_none());
    assert!(pool_entries(catalog, "30920153,4#30920155,0").is_none());
    assert!(pool_entries(catalog, "30920153,4#2147483647,4").is_none());

    let ordinary = pool_entries(catalog, "90151#4150001,10").unwrap();
    assert_eq!(ordinary[0].stacks, 1);
    assert_eq!(ordinary[1].stacks, 10);
    assert!(
        super::super::arsenal::select(catalog, &ordinary)
            .unwrap()
            .is_none()
    );

    let bullets = pool_entries(catalog, "30920153,4#30920155,4").unwrap();
    assert!(
        super::super::arsenal::select(catalog, &bullets)
            .unwrap()
            .is_some()
    );
    let duplicate_type = [bullets[0], bullets[0]];
    assert!(super::super::arsenal::select(catalog, &duplicate_type).is_err());
}
