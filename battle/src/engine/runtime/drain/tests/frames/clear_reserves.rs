use super::*;
use crate::engine::manager::hp::{HpCommand, HpKill};
use sonettobuf::{ActEffect, FightStep, effect_type_enum::EffectType};

#[test]
fn reserve_clear_rejects_a_trailing_argument_separator() {
    let mut behavior = ParsedBehavior::new(60178, "ClearMonsterSub", Vec::new());
    assert!(crate::engine::skill::behavior::is_supported(&behavior));

    behavior.raw_arity = 1;
    assert!(!crate::engine::skill::behavior::is_supported(&behavior));
}

fn effects<'a>(steps: &'a [FightStep], output: &mut Vec<&'a ActEffect>) {
    for effect in steps.iter().flat_map(|step| &step.act_effect) {
        output.push(effect);
        if let Some(child) = effect.fight_step.as_ref() {
            effects(std::slice::from_ref(child), output);
        }
    }
}

#[test]
fn defeat_clears_reserves_before_the_remaining_field_enemies_are_killed() {
    crate::test_support::init_config();
    let entity = |uid, team_type, position| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        position: Some(position),
        current_hp: Some(100),
        ..Default::default()
    };
    let mut owner = entity(-1, 2, 1);
    owner.passive_skill = vec![9_260_002];
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, 1)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![owner, entity(-2, 2, 2)],
            sub_entitys: vec![entity(-7, 2, -1), entity(-3, 2, -2)],
            ..Default::default()
        }),
        version: Some(7),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext {
            current_round: 1,
            ..Default::default()
        },
        [RuleOp::Command(BattleCommand::Hp(HpCommand::Kill(
            HpKill {
                origin: CommandOrigin {
                    domain: RuleDomain::Lifecycle,
                    key: DefinitionKey::new(0, "TestReserveClear"),
                },
                source_uid: 10,
                target_uid: -1,
                config_effect: 1,
            },
        )))],
    )
    .unwrap();

    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let mut projected = Vec::new();
    effects(&steps, &mut projected);
    let clear_index = projected
        .iter()
        .position(|effect| effect.effect_type == Some(EffectType::Clearmonstersub as i32))
        .unwrap();
    let kill_index = projected
        .iter()
        .position(|effect| {
            effect.effect_type == Some(EffectType::Kill as i32) && effect.target_id == Some(-2)
        })
        .unwrap();

    assert!(clear_index < kill_index);
    assert_eq!(projected[clear_index].reserve_str.as_deref(), Some("-7#-3"));
    assert_eq!(projected[clear_index].team_type, Some(2));
    assert_eq!(managers.hp.current(-2), 0);
    let refreshed = pool.runtime_view(&managers);
    for uid in [-7, -3] {
        assert!(refreshed.entity(uid).is_none());
        assert_eq!(managers.hp.current(uid), 100);
    }
}
