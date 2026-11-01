use sonettobuf::Fight;

use crate::{
    catalog::BattleCatalog,
    engine::manager::{
        BattleManagers,
        fight_task::configured_box,
        indicator::{IndicatorClearCommand, IndicatorCommand},
    },
};

#[test]
fn clear_indicator_resets_its_value_without_reducing_activity_score() {
    crate::test_support::init_config();
    let catalog = BattleCatalog::new(crate::test_support::game_data());
    let fight = Fight {
        episode_id: Some(493_701),
        fight_task_box: Some(configured_box(catalog, 493_701).unwrap()),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.execute_indicator(IndicatorCommand {
        indicator_id: 3711,
        amount: 4,
        config_effect: 60016,
    });
    let added = managers.fight_task.take_pending(1).unwrap();
    let added_task = added
        .tasks
        .iter()
        .find(|task| task.task_id == Some(370_101))
        .unwrap();
    assert_eq!(added_task.values[0].progress, Some(4));

    let marker = managers
        .execute_indicator_clear(IndicatorClearCommand {
            indicator_id: 3711,
            config_effect: 60017,
        })
        .unwrap();
    let cleared = managers.fight_task.take_pending(1).unwrap();
    let cleared_task = cleared
        .tasks
        .iter()
        .find(|task| task.task_id == Some(370_101))
        .unwrap();

    assert_eq!(cleared_task.values[0].progress, Some(0));
    assert_eq!(managers.indicator.value(3711), 0);
    assert_eq!(managers.indicator.activity_score(), 4);
    assert_eq!(marker.target_uid, 3711);
    assert_eq!(marker.effect_num, 0);
    assert_eq!(marker.effect_num1, None);
    assert_eq!(marker.config_effect, 60017);

    let restarted = managers
        .execute_indicator(IndicatorCommand {
            indicator_id: 3711,
            amount: 1,
            config_effect: 60016,
        })
        .unwrap();
    assert_eq!(restarted.effect_num, 1);
    assert_eq!(managers.indicator.activity_score(), 5);
}
