use super::*;

#[test]
fn burn_attack_bonus_rounds_before_joining_ordinary_attack() {
    crate::test_support::init_config();
    const SKILL: i32 = 30941131;
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3094),
                current_hp: Some(10_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    attack: Some(1832),
                    ..Default::default()
                }),
                passive_skill: vec![30941142],
                buffs: vec![
                    BuffInfo {
                        uid: Some(20),
                        buff_id: Some(4150001),
                        layer: Some(24),
                        count: Some(1),
                        ..Default::default()
                    },
                    BuffInfo {
                        uid: Some(21),
                        buff_id: Some(433115),
                        layer: Some(6),
                        count: Some(1),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    defense: Some(542),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: SKILL,
    }
    .into();
    invocation.target = SkillTarget::Explicit(-1);
    let mut execution = SkillExecution::new(TargetContext {
        active_skill_source_uid: 10,
        active_skill_is_attack: true,
        ..Default::default()
    });
    execution.configured_targets = Some(vec![-1]);
    execution.planned_crits = Some(vec![(-1, false)]);

    let damage = plan::damage_ops(
        &invocation,
        &managers,
        &pool,
        &catalog,
        SKILL,
        &mut RoundDeterminism::default(),
        &mut execution,
    )
    .unwrap();
    let [HpCommand::Damage(hit)] = damage.damage.as_slice() else {
        panic!("expected a single direct hit");
    };
    assert_eq!(hit.amount, 16_716);
}
