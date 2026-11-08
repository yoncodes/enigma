#[test]
fn exploration_rows_preserve_episode_reward_and_collection_links() {
    crate::init_test_config();
    let tables = crate::configs::get();
    let scene = tables.explore_scene.get(101).unwrap();
    let episode = tables.episode.get(scene.episode_id).unwrap();
    let chest = tables.explore_chest.get(1).unwrap();

    assert_eq!(scene.id, episode.map_id);
    assert_eq!(scene.chapter_id, episode.chapter_id);
    assert_eq!(chest.episode_id, scene.episode_id);
    assert_eq!(chest.chapter_id, scene.chapter_id);
    assert_eq!(chest.is_count, 1);
    assert!(!chest.bonus.is_empty());

    let task = tables.task_explore.get(210151).unwrap();
    assert_eq!(task.listener_type, "ExploreCollect");
    assert_eq!(task.listener_param, format!("{}#2", scene.chapter_id));
    assert_eq!(task.max_progress, 1);
    let item = tables.explore_item.get(150001).unwrap();
    assert_eq!(item.effect, "1#209");
    assert!(!item.is_stackable);
    assert!(!item.is_client_stackable);
}
