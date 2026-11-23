use super::*;

#[test]
fn tracked_target_damage_updates_and_emits_the_indicator() {
    let mut manager = IndicatorManager::default();
    manager.track_damage(IndicatorId::BossRushScore, -1);

    assert!(manager.record_damage(-2, 50).is_none());
    let marker = manager.record_damage(-1, 75).unwrap();

    assert_eq!(marker.target_uid, 4);
    assert_eq!(marker.effect_num, 75);
    assert_eq!(marker.effect_num1, Some(0));
    assert_eq!(marker.config_effect, 60016);
    assert_eq!(manager.total(IndicatorId::BossRushScore), 75);
    assert_eq!(manager.activity_score(), 75);
}

#[test]
fn configured_score_indicators_emit_the_delta() {
    for indicator_id in [
        IndicatorId::BossRushScore,
        IndicatorId::ZongMaoTechniqueScore,
        IndicatorId::Rouge2Boss,
    ] {
        let mut manager = IndicatorManager::default();
        for _ in 0..2 {
            let marker = manager
                .add(IndicatorCommand {
                    indicator_id: indicator_id as i32,
                    amount: 500_000,
                    config_effect: 60016,
                })
                .unwrap();
            assert_eq!(marker.target_uid, indicator_id as i64);
            assert_eq!(marker.effect_num, 500_000);
            assert_eq!(marker.effect_num1, Some(1));
            assert_eq!(marker.config_effect, 60016);
        }
        assert_eq!(manager.totals.get(&(indicator_id as i32)), Some(&1_000_000));
        assert_eq!(manager.activity_score(), 1_000_000);
    }
}

#[test]
fn configured_ordinary_indicator_emits_the_committed_total() {
    let mut manager = IndicatorManager::default();
    for amount in [3_000, 2_000] {
        let marker = manager
            .add(IndicatorCommand {
                indicator_id: 3203,
                amount,
                config_effect: 60016,
            })
            .unwrap();
        assert_eq!(marker.effect_num, manager.totals[&3203] as i32);
    }

    assert_eq!(manager.totals.get(&3203), Some(&5_000));
}
