use super::*;

#[test]
fn auto_round_spends_available_hedone_energy() {
    let free = |skill_id| sonettobuf::CardInfo {
        extra_infos: vec![sonettobuf::CardExtraInfo {
            key: Some(8),
            values: vec!["HedoneCard".to_owned()],
        }],
        ..hand_card(10, skill_id, false)
    };
    let runtime = team_runtime(
        vec![hedone_with_stored_energy()],
        vec![free(31446011), hedone_energy_card()],
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 2);
    assert_eq!(reply.opers[0].param1, Some(2));
    assert_eq!(reply.opers[1].param1, Some(1));
    assert!(
        reply
            .opers
            .iter()
            .all(|operation| operation.to_id == Some(-1))
    );
    assert_eq!(
        reply
            .devices_opers
            .iter()
            .map(|operation| (operation.uid, operation.index))
            .collect::<Vec<_>>(),
        [(Some(10), Some(1))]
    );
}

#[test]
fn auto_round_uses_stored_hedone_energy_for_a_selected_group() {
    let runtime = team_runtime_against(
        vec![hedone_with_stored_energy()],
        (1..=4).map(|index| enemy(-index, 1_000)).collect(),
        vec![hedone_energy_card()],
    );

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(1));
    assert_eq!(reply.devices_opers[0].index, Some(2));
}

#[test]
fn auto_round_uses_stored_hedone_energy_when_the_unique_is_selected() {
    let mut hedone = hedone_with_stored_energy();
    hedone.ex_point = Some(100);
    hedone.ex_point_type = Some(4);
    let mut runtime = team_runtime_against(
        vec![hedone],
        vec![enemy(-1, 1_000)],
        vec![hedone_energy_card()],
    );
    runtime
        .managers
        .conduit
        .execute(ConduitCommand::SelectGroup {
            source_uid: 10,
            group: 3,
        })
        .unwrap();
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60294, "StopDeviceSkill"),
    };
    for skill_id in [31560111, 31560121] {
        runtime
            .managers
            .conduit
            .execute(ConduitCommand::StopSkill {
                origin,
                source_uid: 10,
                team: 1,
                skill_id,
            })
            .unwrap();
    }

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.devices_opers[0].index, Some(3));
    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(1));
}

#[test]
fn auto_round_rejects_missing_hedone_energy_state() {
    let runtime = team_runtime_against(
        vec![team_hero(10, 3156, 1_000, [31560111, 31560121])],
        (1..=4).map(|index| enemy(-index, 1_000)).collect(),
        vec![hedone_energy_card()],
    );

    let error = runtime
        .try_plan_auto_round(&AutoRoundRequest::default())
        .unwrap_err();

    assert!(error.contains("ConduitEnergy(Hedone(InvalidCommand))"));
}

#[test]
fn auto_round_rejects_malformed_hedone_energy_state() {
    let mut hedone = hedone_with_stored_energy();
    hedone.buffs[0].act_info[0].param = vec![2, 9, 1];
    let runtime = team_runtime_against(
        vec![hedone],
        (1..=4).map(|index| enemy(-index, 1_000)).collect(),
        vec![hedone_energy_card()],
    );

    let error = runtime
        .try_plan_auto_round(&AutoRoundRequest::default())
        .unwrap_err();

    assert!(error.contains("ConduitEnergy(Hedone(State("));
    assert!(error.contains("InvalidState"));
}

#[test]
fn auto_round_reads_hedone_power_from_the_configured_carrier() {
    use crate::engine::{
        mechanic::hedone_device_power::available_gain,
        skill::{
            behavior::hedone_device_power::card_release,
            target::{TargetPool, TargetRequest},
        },
    };

    let caster = team_hero(10, 3156, 1_000, [31560111, 31560121]);
    let mut carrier = hedone_with_stored_energy();
    carrier.uid = Some(11);
    carrier.position = Some(2);
    carrier.buffs[0].from_uid = Some(11);
    let mut runtime = team_runtime_against(
        vec![caster, carrier],
        vec![enemy(-1, 100_000)],
        vec![hedone_energy_card()],
    );
    let mut effect = runtime.catalog.get(31560181).unwrap().clone();
    effect.slots[0].target = TargetRequest {
        code: 125,
        raw: Vec::new(),
    };
    effect.slots[0].behavior.args = vec![2];
    effect.slots[0].behavior.raw_args = vec!["2".to_owned()];
    runtime.catalog.insert(effect);
    let pool = TargetPool::from_fight_with_catalog(runtime.managers.catalog(), &runtime.fight);
    let command = card_release(&runtime.managers, &pool, &runtime.catalog, 10, 31560181)
        .unwrap()
        .unwrap();
    let gain = available_gain(&runtime.managers, command).unwrap().unwrap();
    assert_eq!((gain.source_uid, gain.power_id, gain.delta), (11, 2, 2));

    let reply = runtime
        .try_plan_auto_round(&AutoRoundRequest::default())
        .unwrap();

    assert_eq!(reply.opers.len(), 1);
    assert_eq!(reply.opers[0].param1, Some(1));
    assert_eq!(runtime.managers.conduit.power(1, 2), 0);
    let round = runtime
        .advance_round(BeginRoundRequest {
            opers: reply.opers,
            devices_opers: reply.devices_opers,
            ..Default::default()
        })
        .unwrap();
    let release = round
        .fight_step
        .iter()
        .find(|step| step.act_id == Some(31560181))
        .unwrap();
    assert!(release.act_effect.iter().any(|effect| {
        effect.effect_type
            == Some(sonettobuf::effect_type_enum::EffectType::Devicepowerchange as i32)
            && effect.reserve_str.as_deref() == Some("2#2")
    }));
}

#[test]
fn queued_hedone_card_releases_energy_when_the_round_executes() {
    let mut hedone = hedone_with_stored_energy();
    hedone.buffs[0].buff_id = Some(31560181);
    hedone.buffs[0].act_info[0].param = vec![2, 7, 1];
    let mut runtime = team_runtime(
        vec![hedone],
        vec![
            hand_card(10, 31446011, false),
            hedone_energy_card(),
            hand_card(10, 31446012, false),
        ],
    );
    runtime.round_state.act_point = 2;
    let play = BeginRoundOper {
        oper_type: Some(CardOpType::PlayCard.id()),
        param1: Some(1),
        to_id: Some(-1),
        ..Default::default()
    };
    let request = AutoRoundRequest {
        opers: vec![play.clone(), play.clone()],
        ..Default::default()
    };
    let reply = runtime.try_plan_auto_round(&request).unwrap();

    assert_eq!(reply.opers, [play]);
    assert_eq!(runtime.managers.conduit.power(1, 1), 0);
    assert!(runtime.managers.buff.has_buff_id(10, 31560181));

    let round = runtime
        .advance_round(BeginRoundRequest {
            opers: request.opers.into_iter().chain(reply.opers).collect(),
            devices_opers: reply.devices_opers,
            ..Default::default()
        })
        .unwrap();
    let flame = round
        .fight_step
        .iter()
        .find(|step| step.act_id == Some(31560181))
        .expect("the queued Flame Undying was cast");
    assert!(flame.act_effect.iter().any(|effect| {
        effect.effect_type
            == Some(sonettobuf::effect_type_enum::EffectType::Devicepowerchange as i32)
            && effect.reserve_str.as_deref() == Some("1#2")
    }));
    assert!(!runtime.managers.buff.has_buff_id(10, 31560181));
}

#[test]
fn auto_round_keeps_hedones_ready_unique_despite_its_post_action_reset() {
    let mut hedone = team_hero(10, 3156, 1_000, [31560111, 31560121]);
    hedone.ex_point = Some(100);
    hedone.ex_point_max = Some(100);
    hedone.ex_point_type = Some(4);
    hedone.ex_skill = Some(31560131);
    let runtime = team_runtime(vec![hedone], Vec::new());

    let reply = runtime.plan_auto_round(&AutoRoundRequest::default());

    assert_eq!(reply.devices_opers[0].index, Some(3));
}
