use super::*;
use crate::engine::{
    manager::{
        buff::{BuffCommand, BuffGrantChild},
        hp::{HpCommand, HpLoss},
    },
    skill::{
        action::SkillTarget,
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
    },
};

#[test]
fn timed_array_advances_on_the_cast_round_and_removes_its_buffs() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    ex_skill: Some(31020131),
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    team_type: Some(1),
                    current_hp: Some(1_000),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                team_type: Some(2),
                current_hp: Some(1_000),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut cast: crate::engine::skill::action::SkillInvocation =
        crate::engine::skill::action::SkillRequest {
            source_uid: 10,
            skill_id: 31020131,
        }
        .into();
    cast.target = SkillTarget::Explicit(10);
    drain::run_action(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        [],
        cast,
    )
    .unwrap();
    assert_eq!(managers.field.get(1).unwrap().definition.duration, 3);

    let array_buff = managers.buff.buff_id_uid(11, 31020117).unwrap();
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(1, "AddBuff"),
    };
    let unrelated = managers
        .execute_buff(BuffCommand::GrantChild(BuffGrantChild {
            origin,
            source_uid: 10,
            target_uid: 11,
            buff_id: 31020117,
            amount: None,
            params: None,
            act_info: None,
        }))
        .unwrap()
        .change
        .added
        .unwrap()
        .buff
        .uid
        .unwrap();
    managers
        .execute_hp(HpCommand::Lose(HpLoss {
            origin,
            source_uid: -1,
            target_uid: 11,
            amount: 1_000,
            config_effect: 0,
            hurt: None,
        }))
        .unwrap();
    assert_eq!(managers.hp.current(11), 0);

    for remaining in [Some(2), Some(1), None] {
        let result = run_round_end_final_settlement(
            &mut managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext::default(),
            Vec::new(),
        )
        .unwrap();
        assert_eq!(
            managers.field.get(1).map(|field| field.definition.duration),
            remaining,
        );
        assert_eq!(managers.buff.has_buff_id(10, 31020117), remaining.is_some());
        assert_eq!(
            managers.buff.snapshot(11, array_buff).is_some(),
            remaining.is_some(),
        );
        assert!(managers.buff.snapshot(11, unrelated).is_some());
        if remaining.is_none() {
            assert!(result.outcomes.iter().any(|outcome| {
                matches!(outcome, crate::engine::runtime::executor::RuleOutcome::Buff(changes)
                if changes.change.removed.iter().any(|removed| {
                    removed.buff.uid == Some(array_buff) && removed.buff.duration == Some(0)
                }))
            }));
        }
        managers.field.begin_round();
    }
}
