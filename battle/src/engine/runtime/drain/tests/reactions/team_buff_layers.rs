use super::*;
use crate::engine::{
    manager::buff::{BuffCommand, BuffGrant},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain, output::BattleCommand},
};

#[test]
fn team_buff_layers_pay_accepted_enemy_grants_at_settlement() {
    crate::test_support::init_config();
    const BURN: i32 = 4150001;
    const OTHER_BURN: i32 = 31100144;
    const CONVERSION: i32 = 31350131;
    const DYNAMO: i32 = 90071;
    const PASSIVE: i32 = 100;
    const GRANTER: i32 = 101;
    const ACTION: i32 = 102;
    let mut passive = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![CONVERSION, 2]),
        TargetRequest::self_only(),
    );
    passive.condition_target = TargetRequest {
        code: 206,
        raw: Vec::new(),
    };
    passive.conditions = crate::engine::skill::condition::parse_conditions(
        config::configs::get(),
        "761004#4150001#3",
    );
    passive.compiled_route = ConditionRoute::compile(&passive.conditions);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: PASSIVE,
        slots: vec![passive],
    });
    let mut granter = SkillEffectSlot::new(
        ParsedBehavior::new(1, "AddBuff", vec![DYNAMO, 5]),
        TargetRequest::self_only(),
    );
    granter.condition_target = TargetRequest::self_only();
    granter.conditions =
        crate::engine::skill::condition::parse_conditions(config::configs::get(), "579018#1#0");
    granter.compiled_route = ConditionRoute::compile(&granter.conditions);
    catalog.insert(ParsedSkillEffect {
        skill_id: GRANTER,
        slots: vec![granter],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: ACTION,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::new(20002, "AddExPoint", vec![1]),
            TargetRequest::self_only(),
        )],
    });

    let entity = |uid, has_passive| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        passive_skill: if has_passive {
            vec![PASSIVE]
        } else {
            Vec::new()
        },
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                entity(10, true),
                FightEntityInfo {
                    passive_skill: vec![GRANTER],
                    ..entity(11, false)
                },
            ],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    buffs: vec![BuffInfo {
                        uid: Some(21),
                        buff_id: Some(BURN),
                        from_uid: Some(-1),
                        layer: Some(29),
                        ..Default::default()
                    }],
                    ..entity(-1, false)
                },
                entity(-2, false),
                entity(-3, true),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut determinism = RoundDeterminism::default();
    let mut untouched_rng = RoundDeterminism::default();
    let grant = |source_uid, target_uid, buff_id, layer| {
        BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid,
            target_uid,
            buff_id,
            amount: Some(layer),
            occurrences: 1,
            child_uid_reservations: 0,
        })
    };
    let mut apply = |grants| {
        let result = run(
            &mut managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext::default(),
            [RuleOp::Command(BattleCommand::BuffBatch(grants))],
        )
        .unwrap();
        let rewards = result
            .events
            .iter()
            .filter_map(|event| match event {
                BattleEvent::BuffAdded(change) | BattleEvent::BuffChanged(change)
                    if change.buff_id == CONVERSION =>
                {
                    Some((
                        change.target_uid,
                        change.after_amount - change.before_amount,
                    ))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        (rewards, managers.buff.max_id_or_type_layer(10, CONVERSION))
    };

    assert_eq!(apply(vec![grant(-3, -3, BURN, 6)]), (vec![], 0));
    assert_eq!(apply(vec![grant(11, 11, BURN, 6)]), (vec![], 0));
    assert_eq!(
        apply(vec![grant(11, -1, BURN, 5), grant(11, -2, OTHER_BURN, 5)]),
        (vec![(10, 4)], 4),
    );
    assert_eq!(apply(vec![grant(11, -2, OTHER_BURN, 7)]), (vec![], 4));
    assert_eq!(apply(vec![grant(-3, -3, BURN, 3)]), (vec![], 4));
    assert_eq!(apply(vec![grant(11, 11, DYNAMO, 5)]), (vec![(10, 4)], 8));
    assert_eq!(
        apply(vec![grant(11, -2, OTHER_BURN, 2), grant(11, 11, DYNAMO, 5)]),
        (vec![(10, 2)], 10),
    );
    assert_eq!(apply(vec![grant(11, -2, OTHER_BURN, 3)]), (vec![], 10));

    let result = run(
        &mut managers,
        &pool,
        &catalog,
        &mut determinism,
        TargetContext::default(),
        [RuleOp::Skill(
            SkillRequest {
                source_uid: 11,
                skill_id: ACTION,
            }
            .into(),
        )],
    )
    .unwrap();
    let steps = crate::engine::packet::timeline::project(&result.frames).unwrap();
    let [action] = steps.as_slice() else {
        panic!("expected one action");
    };
    let children = action
        .act_effect
        .iter()
        .filter_map(|effect| effect.fight_step.as_ref()?.act_id)
        .collect::<Vec<_>>();
    assert_eq!(children, vec![GRANTER, PASSIVE]);
    assert_eq!(managers.buff.max_id_or_type_layer(10, CONVERSION), 12);
    for _ in 0..4 {
        assert_eq!(
            determinism.lua_random_index(1000),
            untouched_rng.lua_random_index(1000),
        );
    }
}
