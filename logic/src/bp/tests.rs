use super::{
    bonus_red_dots_for_state, bp_time_range, has_task_red_dot, level_purchase_cost,
    parse_bp_reward, select_reward, should_show_task_red_dot, task_score_from_tasks, task_tab_id,
};
use database::db::game::battle_pass::BattlePassState;
use database::db::game::tasks::{TaskLoopType, TaskType};
use sonettobuf::Task;

#[test]
fn appointed_bp_tasks_share_permanent_tab() {
    assert_eq!(
        task_tab_id(TaskLoopType::Appoint),
        TaskLoopType::Permanent.id() as i64
    );
    assert_eq!(
        task_tab_id(TaskLoopType::Daily),
        TaskLoopType::Daily.id() as i64
    );
}

#[test]
fn owned_bp_skin_does_not_drop_other_rewards() {
    let rewards = parse_bp_reward("1#140001#1|5#301703#1", &[301703]);

    assert_eq!(rewards.items, vec![(140001, 1)]);
    assert!(rewards.skins.is_empty());
}

#[test]
fn self_select_reward_index_is_zero_based() {
    let choices = "5#301703#1|5#302003#1|1#622201#1";
    assert_eq!(select_reward(choices, 1), Some("5#302003#1"));
    assert_eq!(select_reward(choices, -1), None);
    assert_eq!(select_reward(choices, 3), None);
}

#[test]
fn bp_level_cost_comes_from_common_config() {
    assert_eq!(level_purchase_cost("2#2#150", 3), Some((2, 450)));
    assert_eq!(level_purchase_cost("1#2#150", 3), None);
}

#[test]
fn weekly_score_cap_hides_non_permanent_task_red_dots() {
    assert!(!should_show_task_red_dot(TaskLoopType::Daily, true));
    assert!(!should_show_task_red_dot(TaskLoopType::Weekly, true));
    assert!(should_show_task_red_dot(TaskLoopType::Permanent, true));
    assert!(should_show_task_red_dot(TaskLoopType::Appoint, true));
    assert!(should_show_task_red_dot(TaskLoopType::Daily, false));
}

#[test]
fn bp_oper_act_tasks_transfer_bp_score() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let bp_id = 26;
    let task = config::configs::get()
        .activity214_task
        .iter()
        .find(|task| task.bp_id == bp_id && task.bonus_score > 0)
        .unwrap();
    let expected_score = task.bonus_score;
    let task = Task {
        id: task.id,
        r#type: Some(TaskType::BpOperAct.id()),
        ..Default::default()
    };

    assert_eq!(task_score_from_tasks(bp_id, &[task]), expected_score);
}

#[test]
fn bp_oper_act_tasks_refresh_bp_task_red_dot() {
    assert!(has_task_red_dot(&[Task {
        r#type: Some(TaskType::BpOperAct.id()),
        ..Default::default()
    }]));
}

#[test]
fn bp_bonus_red_dots_follow_score_payment_and_claim_state() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);
    let bp = config::configs::get().battle_pass(26).unwrap();
    let first = config::configs::get()
        .bp_lv_bonus
        .iter()
        .find(|bonus| bonus.bp_id == bp.bp_id && !bonus.free_bonus.is_empty())
        .unwrap();
    let mut state = BattlePassState {
        score: first.level * bp.exp_level_up,
        weekly_score: 0,
        pay_status: 0,
        first_show: false,
        sp_first_show: false,
        has_get_self_select_bonus: Vec::new(),
        has_get_free_bonus: Vec::new(),
        has_get_pay_bonus: Vec::new(),
        has_get_sp_free_bonus: Vec::new(),
        has_get_sp_pay_bonus: Vec::new(),
    };

    assert_eq!(
        bonus_red_dots_for_state(bp.bp_id, bp.exp_level_up, &state).normal,
        1
    );

    state.has_get_free_bonus = config::configs::get()
        .bp_lv_bonus
        .iter()
        .filter(|bonus| bonus.bp_id == bp.bp_id && bonus.level <= first.level)
        .map(|bonus| bonus.level)
        .collect();
    assert_eq!(
        bonus_red_dots_for_state(bp.bp_id, bp.exp_level_up, &state).normal,
        0
    );

    state.pay_status = 1;
    assert_eq!(
        bonus_red_dots_for_state(bp.bp_id, bp.exp_level_up, &state).normal,
        i32::from(config::configs::get().bp_lv_bonus.iter().any(|bonus| {
            bonus.bp_id == bp.bp_id && bonus.level <= first.level && !bonus.pay_bonus.is_empty()
        }))
    );
}

#[test]
fn bp_time_range_ignores_blank_task_windows() {
    let data_dir = format!("{}/../data/excel2json", env!("CARGO_MANIFEST_DIR"));
    let _ = config::init(&data_dir);

    assert_eq!(
        bp_time_range(26),
        (Some(1_786_615_200), Some(1_790_157_599))
    );
}
