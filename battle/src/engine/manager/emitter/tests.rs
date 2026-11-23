use sonettobuf::{Fight, FightEntityInfo, FightTeam};

use super::*;
use crate::engine::{
    manager::hp::{HpCommand, MaxHpAdjust},
    skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    skill::target::TargetPool,
};

fn definition() -> ImpromptuDefinition {
    ImpromptuDefinition::new(11, 22, 150)
}

const ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::BuffAct,
    key: DefinitionKey::new(875, "EmitterTag"),
};

#[test]
fn activation_registers_hp_presence_without_replacing_virtual_stats() {
    crate::test_support::init_config();
    let entity = |uid, attack| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            attack: Some(attack),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10, 100), entity(11, 300)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);

    managers.execute_emitter(EmitterCommand {
        origin: ORIGIN,
        operation: EmitterOperation::Enable(definition()),
    });

    assert_eq!(managers.hp.current(UID), 1);
    assert_eq!(managers.hp.max(UID), 0);
    assert!(managers.entity.snapshot(UID).is_none());
    assert_eq!(
        pool.runtime_view(&managers).entity(UID).unwrap().attack,
        200
    );

    managers
        .hp
        .execute_command(HpCommand::AdjustMax(MaxHpAdjust {
            origin: ORIGIN,
            source_uid: 10,
            target_uid: UID,
            delta: 1,
        }))
        .unwrap();
    managers.execute_emitter(EmitterCommand {
        origin: ORIGIN,
        operation: EmitterOperation::Enable(definition()),
    });

    assert_eq!(managers.hp.current(UID), 1);
    assert_eq!(managers.hp.max(UID), 1);
}

#[test]
fn activation_identity_matches_the_captured_emitter_shell() {
    let entity = activation_entity();

    assert_eq!(entity.uid, Some(UID));
    assert_eq!(entity.entity_type, Some(6));
    assert_eq!(entity.team_type, Some(1));
    assert_eq!(entity.current_hp, Some(1));
    assert_eq!(entity.ex_point_max, Some(0));
    assert_eq!(entity.attr.unwrap().hp, Some(0));
    assert_eq!(entity.base_attr.unwrap().hp, Some(0));
    assert_eq!(entity.guard, Some(-1));
    assert_eq!(entity.enhance_info_box.unwrap().uid, Some(UID));
}

#[test]
fn queues_only_when_played_cards_produce_inspiration() {
    let mut emitter = EmitterManager::default();
    emitter.enable(definition());
    let played = [PlayedCard {
        card: CardInfo {
            energy: Some(2),
            ..Default::default()
        },
        caster_uid: 0,
        card_index: 1,
        hand_index: None,
        skill_id: 1,
        uses_action_point: false,
        rank_change_pending: false,
        rewritten: false,
        target_uid: None,
        recorded_skill: None,
    }];

    assert!(emitter.queue_card(played.len()).is_none());
    assert_eq!(emitter.collect_energy(&played).unwrap().applied_delta, 2);
    assert!(emitter.queue_card(played.len()).is_some());
    assert_eq!(emitter.finish_skill().applied_delta, -2);
    assert!(emitter.queue_card(played.len()).is_none());
}

#[test]
fn inspiration_uses_the_applied_eureka_spend() {
    let mut emitter = EmitterManager::default();
    assert_eq!(emitter.inspiration_from_eureka(-2), 0);

    emitter.enable(definition());
    assert_eq!(emitter.inspiration_from_eureka(-1), 1);
    assert_eq!(emitter.inspiration_from_eureka(2), 0);
}

#[test]
fn impromptu_damage_rate_uses_all_collected_inspiration() {
    let mut emitter = EmitterManager::default();
    emitter.enable(definition());
    emitter.add_energy(6);

    assert_eq!(
        emitter.skill_rate(UID, definition().skill_id()),
        Some(EmitterSkillRate {
            act_id: definition().damage_up_act_id(),
            rate: 900,
        })
    );
    assert_eq!(emitter.skill_rate(1, definition().skill_id()), None);
}

#[test]
fn emitter_definition_is_resolved_from_config_types() {
    crate::test_support::init_config();
    let db = config::get();
    let definition = crate::catalog::impromptu_definition(db).unwrap();

    assert_eq!(
        definition.skill_id(),
        db.fight_asfd_const
            .get(5)
            .unwrap()
            .value
            .parse::<i32>()
            .unwrap()
    );
    assert_eq!(
        db.buff_act
            .get(definition.damage_up_act_id())
            .unwrap()
            .r#type,
        "EmitterDamageUp"
    );
}

#[test]
fn emitter_averages_stored_ally_buff_attributes_including_critical_stats() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(11)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
    managers.buff.add(&managers.hp, 10, 10, 31080121, 0);
    managers.buff.add(&managers.hp, 11, 11, 31080122, 0);
    managers.buff.add(&managers.hp, 10, 10, 31080145, 0);
    managers.buff.add(&managers.hp, 11, 11, 31080146, 0);

    assert_eq!(
        average_ally_attribute_delta(
            &managers.emitter,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
            AttrId::DmgBonus,
        ),
        275
    );
    assert_eq!(
        average_ally_attribute_delta(
            &managers.emitter,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
            AttrId::CriticalRate,
        ),
        37
    );
    assert_eq!(
        average_ally_attribute_delta(
            &managers.emitter,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
            AttrId::CriticalDmg,
        ),
        37
    );
}

#[test]
fn emitter_snapshot_excludes_after_round_start_team_energy_attributes() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(11)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
    let added = managers
        .buff
        .add(&managers.hp, 10, 10, 31080143, 0)
        .unwrap();
    managers
        .execute_buff(crate::engine::manager::buff::BuffCommand::SetState(
            crate::engine::manager::buff::BuffSetState {
                ex_info: None,
                origin: crate::engine::skill::buff_act::configured_command_origin(
                    882,
                    crate::engine::skill::buff_act::registry::BuffActKind::FixAttrTeamEnergy,
                )
                .unwrap(),
                target_uid: 10,
                buff_uid: added.buff.uid.unwrap(),
                params: Some("882#10".into()),
                act_info: None,
            },
        ))
        .unwrap();

    assert_eq!(
        average_ally_attribute_delta(
            &managers.emitter,
            &managers.attribute,
            &managers.buff,
            &managers.hp,
            AttrId::DmgBonus,
        ),
        0
    );
}

#[test]
fn emitter_snapshot_includes_existing_conditional_shield_attributes() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(10), entity(11)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = crate::engine::manager::BattleManagers::seeded(&fight);
    managers.buff.add(&managers.hp, 10, 10, 31170002, 0);
    managers.buff.add(&managers.hp, 11, 11, 31170002, 0);
    managers.hp.set_shield(10, 6000);
    managers.hp.set_shield(11, 3000);

    managers
        .emitter
        .snapshot_ally_attributes(&managers.buff, &managers.hp);

    assert_eq!(managers.emitter.ally_attribute(AttrId::DmgBonus), 90);
}
