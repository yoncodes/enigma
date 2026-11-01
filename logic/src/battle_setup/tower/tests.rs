use std::path::{Path, PathBuf};

use database::models::game::tower::TowerType;
use sonettobuf::{StartDungeonRequest, StartTowerBattleRequest};

use super::validate_battle_start;

fn tables() -> &'static config::GameDB {
    let data = std::env::var_os("ENIGMA_BATTLE_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../data/excel2json"));
    config::init(data.to_str().unwrap()).unwrap();
    config::configs::get()
}

fn request(
    tower_type: TowerType,
    tower_id: i32,
    layer_id: i32,
    difficulty: i32,
    episode_id: i32,
) -> StartTowerBattleRequest {
    StartTowerBattleRequest {
        start_dungeon_request: Some(StartDungeonRequest {
            episode_id: Some(episode_id),
            ..Default::default()
        }),
        r#type: Some(tower_type.id()),
        tower_id: Some(tower_id),
        layer_id: Some(layer_id),
        difficulty: Some(difficulty),
        talent_plan_id: Some(0),
    }
}

#[test]
fn tower_episode_identity_is_derived_from_each_config_family() {
    let tables = tables();

    let permanent = tables.tower_permanent_episode.iter().next().unwrap();
    let permanent_episode = permanent
        .episode_ids
        .split('|')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        validate_battle_start(
            tables,
            &request(
                TowerType::Normal,
                0,
                permanent.layer_id,
                0,
                permanent_episode,
            ),
        )
        .is_ok()
    );

    let boss = tables.tower_boss_episode.iter().next().unwrap();
    assert!(
        validate_battle_start(
            tables,
            &request(
                TowerType::Boss,
                boss.tower_id,
                boss.layer_id,
                0,
                boss.episode_id,
            ),
        )
        .is_ok()
    );

    let teach = tables.tower_boss_teach.iter().next().unwrap();
    assert!(
        validate_battle_start(
            tables,
            &request(
                TowerType::Boss,
                teach.tower_id,
                0,
                teach.teach_id,
                teach.episode_id,
            ),
        )
        .is_ok()
    );

    let limited = tables.tower_limited_episode.iter().next().unwrap();
    assert!(
        validate_battle_start(
            tables,
            &request(
                TowerType::Limited,
                limited.season,
                limited.layer_id,
                limited.difficulty,
                limited.episode_id,
            ),
        )
        .is_ok()
    );
}

#[test]
fn related_but_mismatched_tower_coordinates_are_rejected() {
    let tables = tables();
    let boss = tables.tower_boss_episode.iter().next().unwrap();
    let mismatched = request(
        TowerType::Boss,
        boss.tower_id,
        boss.layer_id + 1,
        0,
        boss.episode_id,
    );

    assert!(validate_battle_start(tables, &mismatched).is_err());
}

#[test]
fn echo_tutorial_enforces_its_own_boss_and_fixed_talent_plan() {
    let tables = tables();
    for teach in tables.tower_boss_teach.iter() {
        let boss = tables.tower_boss.iter().find(|row| row.tower_id == teach.tower_id).unwrap();
        let mut req = request(TowerType::Boss, teach.tower_id, 0, teach.teach_id, teach.episode_id);
        req.start_dungeon_request.as_mut().unwrap().fight_group = Some(sonettobuf::FightGroup::default());
        let (dungeon, context) = validate_battle_start(tables, &req).unwrap();
        assert_eq!(context.talent_plan_id, teach.plan_id);
        assert_eq!(dungeon.fight_group.unwrap().assist_boss_id, Some(boss.boss_id));
        req.start_dungeon_request.as_mut().unwrap().fight_group.as_mut().unwrap().assist_boss_id = Some(999999);
        assert!(validate_battle_start(tables, &req).is_err());
    }
}

#[tokio::test]
async fn all_echo_tutorials_build_with_their_configured_trials_and_boss() {
    use sonettobuf::{FightGroup, TrialHero};
    let tables = tables();
    let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1)
        .connect("sqlite::memory:").await.unwrap();
    database::run_migrations(&db).await.unwrap();
    sqlx::query("INSERT INTO users (id, username, created_at, updated_at) VALUES (977001, 'echo-trial', 0, 0)")
        .execute(&db).await.unwrap();
    let teaches = tables.tower_boss_teach.iter().collect::<Vec<_>>();
    assert_eq!(teaches.len(), 12);
    for teach in teaches {
        let boss = tables.tower_boss.iter().find(|row| row.tower_id == teach.tower_id).unwrap();
        let episode = tables.episode.get(teach.episode_id).unwrap();
        let battle = tables.battle.get(episode.battle_id).unwrap();
        let trials = battle.trial_heros.split('|').map(|entry| {
            let parts = entry.split('#').collect::<Vec<_>>();
            TrialHero { trial_id: Some(parts[0].parse().unwrap()), pos: Some(parts[2].parse().unwrap()),
                ..Default::default() }
        }).collect::<Vec<_>>();
        let mut req = request(TowerType::Boss, teach.tower_id, 0, teach.teach_id, teach.episode_id);
        req.talent_plan_id = Some(1);
        req.start_dungeon_request.as_mut().unwrap().fight_group = Some(FightGroup {
            assist_boss_id: Some(boss.boss_id), trial_hero_list: trials.clone(), ..Default::default()
        });
        let (dungeon, context) = validate_battle_start(tables, &req).unwrap();
        let built = super::build_fight(&db, 977001, teach.episode_id, episode.battle_id,
            dungeon.fight_group.as_ref().unwrap(), battle::dungeon::FightOptions::default(), context)
            .await.unwrap_or_else(|error| panic!("teach {} could not start: {error:#}", teach.teach_id));
        let team = built.fight.attacker.as_ref().unwrap();
        let support = team.assist_boss.as_ref().expect("tutorial needs its assist boss");
        assert_eq!(support.model_id, Some(boss.boss_id));
        assert_eq!(support.level, Some(super::tower_const(tables, database::models::game::tower::TowerConstId::TeachBossLevel).unwrap()));
        assert_eq!(team.entitys.len(), trials.len());
        let mut ids = team.entitys.iter().filter_map(|e| e.uid).collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), trials.len());
        assert!(ids.insert(support.uid.unwrap()), "support and trial heroes must have distinct uids");
        for enemy in &built.fight.defender.as_ref().unwrap().entitys {
            assert!(ids.insert(enemy.uid.unwrap()), "enemy uid must not collide with an ally");
        }
        assert_eq!(context.talent_plan_id, teach.plan_id, "teach plan must override owned plan");
        let mut runtime = battle::Battle::start(battle::catalog::BattleCatalog::new(tables), built, 42)
            .unwrap_or_else(|error| panic!("teach {} could not initialize battle: {error}", teach.teach_id));
        let hand = runtime.card_info_push().card_group;
        let card_index = hand.iter().position(|card| {
            let skill_id = card.skill_id.unwrap_or_default();
            let effect_id = tables.skill.get(skill_id).map(|skill| skill.skill_effect).unwrap_or(skill_id);
            tables.skill_effect.get(effect_id).is_some_and(|effect| effect.damage_rate > 0)
        }).unwrap_or_else(|| panic!("teach {} has no attack in initial hand", teach.teach_id));
        let target = runtime.reconnect_state().0.defender.unwrap().entitys[0].uid.unwrap();
        let reply = battle::dungeon::begin_round(&mut runtime, sonettobuf::BeginRoundRequest {
            opers: vec![sonettobuf::BeginRoundOper {
                oper_type: Some(battle::tooling::replay::CardOpType::PlayCard.id()),
                param1: Some(card_index as i32 + 1), to_id: Some(target), ..Default::default()
            }], ..Default::default()
        }).unwrap_or_else(|error| panic!("teach {} first attack failed: {error}", teach.teach_id));
        assert!(reply.round.is_some());
    }
    let owned: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_assist_bosses WHERE user_id = 977001")
        .fetch_one(&db).await.unwrap();
    assert_eq!(owned, 0, "the tutorial must not unlock a permanent boss");
}
