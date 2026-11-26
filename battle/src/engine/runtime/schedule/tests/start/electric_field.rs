use super::*;
use crate::engine::{
    event::payload::BattleEvent,
    manager::field::FieldChangeKind,
    skill::effect::slot::{ParsedBehavior, ParsedSkillEffect},
};

#[test]
fn electric_field_precedes_both_sides_early_opening_rules() {
    init_config();
    let entity = |uid, team_type, passive_skill| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(team_type),
        current_hp: Some(100),
        passive_skill,
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 1, vec![31350161, 116331900])],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 2, vec![103374116])],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut catalog = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut early_ally_rule = catalog.get(116331900).unwrap().clone();
    early_ally_rule.slots[0].condition_target = TargetRequest::self_only();
    early_ally_rule.slots[0].target = TargetRequest::self_only();
    early_ally_rule.slots[0].target_from_condition = false;
    catalog.insert(early_ally_rule);
    let (opening, _) = run_start(
        managers.catalog(),
        BattleAccess::new(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                current_round: 1,
                ..Default::default()
            },
        ),
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    let deployment = opening
        .events
        .iter()
        .enumerate()
        .filter_map(|(i, event)| match event {
            BattleEvent::FieldChanged(change)
                if change.kind == FieldChangeKind::Deployed && change.field_id == 30001 =>
            {
                Some(i)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(deployment.len(), 1);
    assert_eq!(managers.field.get(1).unwrap().create_uid, 10);
    for buff_id in [116331900, 103374118] {
        let grant = opening
            .events
            .iter()
            .position(|event| matches!(event, BattleEvent::BuffAdded(change) if change.buff_id == buff_id))
            .unwrap_or_else(|| panic!("no opening grant for {buff_id}"));
        assert!(
            deployment[0] < grant,
            "{buff_id} was granted before the field"
        );
    }
}

#[test]
fn electric_field_attached_after_the_early_pass_still_runs() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(100),
                passive_skill: vec![8],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let source = SkillEffectCatalog::from_game_db(config::configs::get());
    let mut grant = source.get(103374116).unwrap().slots[2].clone();
    grant.behavior = ParsedBehavior::new(1, "AddBuff", vec![423611]);
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 8,
        slots: vec![grant],
    });
    catalog.insert(ParsedSkillEffect {
        skill_id: 423621,
        slots: vec![source.get(31350161).unwrap().slots[1].clone()],
    });
    let (opening, _) = run_start(
        managers.catalog(),
        BattleAccess::new(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                current_round: 1,
                ..Default::default()
            },
        ),
        CardSetup {
            hand: Vec::new(),
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        0,
    )
    .unwrap();

    let grant = opening
        .events
        .iter()
        .position(
            |event| matches!(event, BattleEvent::BuffAdded(change) if change.buff_id == 423611),
        )
        .unwrap();
    let deployment = opening
        .events
        .iter()
        .position(|event| {
            matches!(event, BattleEvent::FieldChanged(change) if change.kind == FieldChangeKind::Deployed)
        })
        .unwrap();
    assert!(grant < deployment);
    assert_eq!(managers.field.get(1).unwrap().create_uid, 10);
}
