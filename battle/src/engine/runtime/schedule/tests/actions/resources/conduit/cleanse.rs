use super::*;
use crate::engine::{
    event::{dispatcher::dispatch_event, payload::BattleEvent},
    skill::action::SkillPhase,
};

#[test]
fn conduit_unique_cast_dispels_the_casters_negative_status() {
    init_config();
    let status = |uid| BuffInfo {
        uid: Some(uid),
        buff_id: Some(109390005),
        from_uid: Some(-1),
        layer: Some(2),
        ..Default::default()
    };
    let fight = Fight {
        battle_id: Some(109390001),
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3149),
                ex_point_type: Some(
                    crate::engine::manager::ex_point::ExPointKind::DevicePower.as_wire(),
                ),
                ex_point: Some(100),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    attack: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![status(20)],
                passive_skill: vec![109390022],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(-1),
                model_id: Some(1001),
                current_hp: Some(100_000),
                attr: Some(HeroAttribute {
                    hp: Some(100_000),
                    ..Default::default()
                }),
                buffs: vec![status(21)],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    for (group, position, skill_id, remaining) in [
        (1, 1, 31490111, 2),
        (1, 2, 31490121, 2),
        (3, 1, 31490151, 0),
    ] {
        managers
            .conduit
            .execute(
                crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                    source_uid: 10,
                    group,
                },
            )
            .unwrap();
        let result = crate::engine::runtime::drain::run_conduit_action(
            BattleAccess::new(
                &mut managers,
                &pool,
                &catalog,
                &mut RoundDeterminism::default(),
                TargetContext::default(),
            ),
            10,
            group,
            crate::engine::runtime::drain::ConduitSlot {
                device_index: 1,
                skill_position: position,
            },
            skill_id,
            None,
            None,
        )
        .unwrap();

        assert_eq!(managers.buff.buff_id_amount(10, 109390005), remaining);
        assert_eq!(managers.buff.buff_id_amount(-1, 109390005), 2);
        if remaining != 0 {
            continue;
        }
        let mut phases = Vec::new();
        for event in &result.events {
            let BattleEvent::SkillAction(action) = event else {
                continue;
            };
            if action.source_uid != 10 || action.skill_id != skill_id {
                continue;
            }
            let dispatch = dispatch_event(
                &pool,
                &managers,
                &catalog,
                &mut RoundDeterminism::default(),
                event,
            )
            .unwrap();
            phases.extend(
                dispatch
                    .passes()
                    .flat_map(|batch| &batch.skills)
                    .filter(|(subscriber, _)| {
                        subscriber.owner_uid == 10 && subscriber.skill_id == 109390022
                    })
                    .map(|_| action.phase),
            );
        }
        assert_eq!(phases, [SkillPhase::AfterHit]);
    }
}
