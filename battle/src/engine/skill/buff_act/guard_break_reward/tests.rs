use crate::engine::{
    manager::{
        BattleManagers,
        hp::{DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData},
        toughness::ToughnessRecover,
    },
    runtime::{determinism::RoundDeterminism, drain, schedule},
    skill::{
        effect::SkillEffectCatalog,
        rule::{
            CommandOrigin, DefinitionKey, RuleDomain,
            output::{BattleCommand, RuleOp},
        },
        target::{TargetContext, TargetPool},
    },
};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

fn fixture(enemy_hp: i32) -> (BattleManagers, TargetPool, SkillEffectCatalog) {
    crate::test_support::init_config();
    let actor = |uid, model_id, team_type, hp| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        team_type: Some(team_type),
        current_hp: Some(hp),
        attr: Some(HeroAttribute {
            hp: Some(hp),
            ..Default::default()
        }),
        toughness_value: Some(100),
        toughness_point: Some(1),
        is_broken: Some(false),
        ..Default::default()
    };
    let mut owner = actor(10, 3147, 1, 10000);
    owner.buffs = vec![BuffInfo {
        uid: Some(20),
        buff_id: Some(31471009),
        from_uid: Some(10),
        ..Default::default()
    }];
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![owner, actor(11, 3146, 1, 10000)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![actor(-1, 1, 2, enemy_hp)],
            ..Default::default()
        }),
        ..Default::default()
    };
    (
        BattleManagers::seeded(&fight),
        TargetPool::from_fight(&fight),
        SkillEffectCatalog::from_fight(config::configs::get(), &fight),
    )
}

fn damage(source_uid: i64, target_uid: i64) -> BattleCommand {
    BattleCommand::Hp(HpCommand::Damage(HpDamage {
        origin: CommandOrigin {
            domain: RuleDomain::Skill,
            key: DefinitionKey::new(1, "SkillDamage"),
        },
        source_uid,
        target_uid,
        amount: 100,
        config_effect: 0,
        effect_kind: DamageEffectKind::Normal,
        assassinate: false,
        ignore_riposte: false,
        hurt: HurtInfoData {
            from_uid: source_uid,
            is_crit: false,
            career_restraint: true,
            reduce_hp: 0,
            effect_id: 0,
            skill_id: 1,
            damage_from: HurtDamageFromType::Skill,
            buff_act_id: 0,
            buff_uid: 0,
            hurt_effect_type: 0,
            display_amount: None,
        },
    }))
}

fn run(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    commands: impl IntoIterator<Item = BattleCommand>,
) {
    drain::run_command_group(
        managers,
        pool,
        catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        commands.into_iter().map(RuleOp::Command),
    )
    .unwrap();
}

#[test]
fn guard_breaks_reward_at_settlement() {
    let (mut managers, pool, catalog) = fixture(10000);
    let db = config::configs::get();
    for raw in ["1136#31470001#1", "1136#31470001#2"] {
        assert!(
            super::super::registry::resolve_feature(Some(db), raw)
                .unwrap()
                .arguments_supported
        );
    }
    assert!(
        !super::super::registry::resolve_feature(Some(db), "1136#31420001#1")
            .unwrap()
            .arguments_supported
    );
    run(
        &mut managers,
        &pool,
        &catalog,
        [damage(11, -1), damage(-1, 10)],
    );
    assert_eq!(managers.buff.buff_id_amount(10, 31470001), 0);
    assert_eq!(managers.buff.act_value(20, 1136), 1);
    let settle = |managers: &mut BattleManagers| {
        schedule::run_attacker_round_end(
            managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
        )
        .unwrap();
    };
    settle(&mut managers);
    assert_eq!(managers.buff.buff_id_amount(10, 31470001), 2);
    assert_eq!(managers.buff.act_value(20, 1136), 0);
    settle(&mut managers);
    assert_eq!(managers.buff.buff_id_amount(10, 31470001), 2);
    for _ in 0..6 {
        run(
            &mut managers,
            &pool,
            &catalog,
            [
                BattleCommand::ToughnessRecover(ToughnessRecover {
                    origin: CommandOrigin {
                        domain: RuleDomain::Behavior,
                        key: DefinitionKey::new(60287, "ToughnessRecover"),
                    },
                    target_uid: -1,
                    config_effect: 60287,
                }),
                damage(11, -1),
            ],
        );
    }
    assert_eq!(managers.buff.buff_id_amount(10, 31470001), 2);
    settle(&mut managers);
    assert_eq!(managers.buff.buff_id_amount(10, 31470001), 10);
    assert_eq!(managers.buff.act_value(20, 1136), 0);
}

#[test]
fn final_guard_break_rewards_the_winning_side() {
    let (mut managers, pool, catalog) = fixture(100);
    run(&mut managers, &pool, &catalog, [damage(11, -1)]);
    assert_eq!(managers.hp.current(-1), 0);
    assert_eq!(managers.buff.buff_id_amount(10, 31470001), 0);
    let victory = crate::engine::round::outcome::BattleOutcome::Victory;
    assert!(managers.commit_terminal(victory));
    assert_eq!(managers.terminal_outcome(), Some(victory));
    schedule::run_finished_attacker_settlement(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
    )
    .unwrap();
    assert_eq!(managers.buff.buff_id_amount(10, 31470001), 2);
    assert_eq!(managers.buff.act_value(20, 1136), 0);
}

#[test]
fn existing_channel_runs_at_small_round_end() {
    crate::test_support::init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3104),
                team_type: Some(1),
                current_hp: Some(1000),
                attr: Some(HeroAttribute {
                    hp: Some(1000),
                    attack: Some(100),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31040003),
                    from_uid: Some(10),
                    ex_info: Some(0),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1),
                team_type: Some(2),
                current_hp: Some(10000),
                attr: Some(HeroAttribute {
                    hp: Some(10000),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    schedule::run_attacker_round_end(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
    )
    .unwrap();
    assert_eq!(managers.buff.snapshot(10, 20).unwrap().ex_info, Some(1));
    assert_eq!(managers.buff.buff_id_amount(-1, 31040001), 1);
}
