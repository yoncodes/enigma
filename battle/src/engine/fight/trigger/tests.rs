use super::*;
use crate::test_support::init_config;

#[test]
fn battle_nine_wave_one_resolves_its_configured_prompt() {
    init_config();
    let actions = wave_start_actions(config::configs::get(), 9_001_101, 1).unwrap();

    assert_eq!(
        crate::catalog::BattleCatalog::new(config::configs::get())
            .wave_start_actions(9_001_101, 1)
            .unwrap(),
        actions
    );

    assert_eq!(
        actions,
        vec![WaveStartAction {
            trigger_id: 330_102,
            action_id: 33_010_201,
            kind: TriggerActionKind::Prompt,
        }]
    );
}

#[test]
fn battle_nine_prompt_does_not_run_for_wave_two() {
    init_config();
    assert!(
        wave_start_actions(config::configs::get(), 9_001_101, 2)
            .unwrap()
            .is_empty()
    );
}
