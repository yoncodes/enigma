use super::*;
use crate::engine::manager::field::FieldChangeKind;

#[test]
fn electric_field_finishes_removal_before_the_next_team_ticks() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1_000),
                passive_skill: vec![31430144],
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(31430142),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(11),
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
    for (team, create_uid, field_id, initial_level) in [(1, 10, 30003, 3), (2, 11, 30001, 1)] {
        let duration = managers.catalog().magic_circle(field_id).unwrap().duration;
        managers
            .field
            .execute_command(FieldCommand {
                origin: CommandOrigin {
                    domain: RuleDomain::Behavior,
                    key: DefinitionKey::new(50019, "AddMagicCircle"),
                },
                team,
                operation: FieldOperation::DeployIfAbsent {
                    definition: FieldDefinition { field_id, duration },
                    create_uid,
                    initial_level,
                    thresholds: Vec::new(),
                },
            })
            .unwrap();
    }

    let mut determinism = RoundDeterminism::default();
    let mut settle = |managers: &mut BattleManagers| {
        run_round_end_final_settlement(
            managers,
            &pool,
            &catalog,
            &mut determinism,
            TargetContext::default(),
            Vec::new(),
        )
        .unwrap()
    };
    let first = settle(&mut managers);
    assert_eq!(managers.field.get(1).unwrap().definition.duration, 1);
    assert_eq!(managers.field.get(2).unwrap().definition.duration, 2);
    assert!(managers.buff.has_buff_id(10, 31430142));
    assert!(first.events.iter().any(|event| {
        matches!(event, BattleEvent::FieldChanged(change) if change.kind == FieldChangeKind::Duration)
    }));

    let last = settle(&mut managers);
    assert!(managers.field.get(1).is_none());
    assert_eq!(managers.field.get(2).unwrap().definition.duration, 1);
    assert!(!managers.buff.has_buff_id(10, 31430142));
    let removal = last.events.iter().position(|event| {
        matches!(event, BattleEvent::FieldChanged(change) if change.kind == FieldChangeKind::Removed)
    }).unwrap();
    let cleanup = last
        .events
        .iter()
        .position(
            |event| matches!(event, BattleEvent::BuffRemoved(change) if change.buff_id == 31430142),
        )
        .unwrap();
    let tick = last.events.iter().position(|event| {
        matches!(event, BattleEvent::FieldChanged(change) if change.team == 2 && change.kind == FieldChangeKind::Duration)
    }).unwrap();
    assert!(removal < cleanup && cleanup < tick);
    assert!(
        last.frames
            .iter()
            .any(|root| root.items.windows(3).any(|items| matches!(
                items,
                [FrameItem::Child(expired), FrameItem::Child(passive), FrameItem::Child(ticking)]
                    if expired.owner == FrameOwner::ElectricFieldDuration
                        && matches!(passive.owner, FrameOwner::Skill { skill_id: 31430144, .. })
                        && ticking.owner == FrameOwner::ElectricFieldDuration
            ))),
        "{:#?}",
        last.frames
    );
}
