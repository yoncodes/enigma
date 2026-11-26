use super::*;
use crate::engine::{
    manager::field::{FieldCommand, FieldDefinition, FieldOperation},
    runtime::record::FrameItem,
};

#[test]
fn ultimate_selects_field_casts_once_per_configured_slot_group() {
    crate::test_support::init_config();
    let entity = |uid, model_id| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        position: Some(1),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(10),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 3132)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                buffs: vec![BuffInfo {
                    buff_id: Some(31320129),
                    uid: Some(99),
                    from_uid: Some(10),
                    ..Default::default()
                }],
                ..entity(-1, 103370111)
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let catalog =
        SkillEffectCatalog::from_roots(config::configs::get(), [31320132, 31320135, 31410151], []);
    let pool = TargetPool::from_fight(&fight);
    let self_cast = |skill_id| (10, skill_id, Some(10));
    for (ultimate, circle, level, expected) in [
        (31320132, None, 0, vec![self_cast(31320150)]),
        (
            31320132,
            Some(40002),
            2,
            vec![(10, 31410151, Some(-1)), self_cast(31320150)],
        ),
        (31320132, Some(30001), 1, vec![self_cast(31320151)]),
        (31320132, Some(30002), 2, vec![self_cast(31320152)]),
        (31320132, Some(30003), 3, vec![self_cast(31320153)]),
        (
            31320135,
            Some(30003),
            3,
            vec![self_cast(31320164), self_cast(31320164)],
        ),
    ] {
        let mut managers = BattleManagers::seeded(&fight);
        if let Some(field_id) = circle {
            managers
                .field
                .execute_command(FieldCommand {
                    origin: CommandOrigin {
                        domain: RuleDomain::Behavior,
                        key: DefinitionKey::new(50019, "AddMagicCircle"),
                    },
                    team: 1,
                    operation: FieldOperation::DeployReplacement {
                        definition: FieldDefinition {
                            field_id,
                            duration: 3,
                        },
                        owner_uid: 10,
                        create_uid: 10,
                        level,
                    },
                })
                .unwrap();
        }
        let cast = SkillInvocation {
            mode: SkillExecutionMode::Direct,
            target: SkillTarget::Explicit(-1),
            ..SkillRequest {
                source_uid: 10,
                skill_id: ultimate,
            }
            .into()
        };
        let result = run_action(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext::default(),
            [],
            cast,
        )
        .unwrap();
        let parent = result
            .frames
            .iter()
            .find(|frame| matches!(frame.owner, FrameOwner::Skill { skill_id, .. } if skill_id == ultimate))
            .unwrap();
        let children: Vec<_> = parent
            .items
            .iter()
            .filter_map(|item| match item {
                FrameItem::Child(child) => match child.owner {
                    FrameOwner::Skill {
                        source_uid,
                        skill_id,
                        target_uid,
                        ..
                    } => Some((source_uid, skill_id, target_uid)),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        assert_eq!(children, expected, "ultimate {ultimate}, field {circle:?}");
    }
}
