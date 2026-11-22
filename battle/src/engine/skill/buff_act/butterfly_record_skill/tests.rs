use super::*;
use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    manager::buff::BuffGrant,
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        rule::{DefinitionKey, RuleDomain},
        target::TargetContext,
    },
};
use sonettobuf::{Fight, FightEntityInfo, FightTeam};

#[test]
fn selection_keeps_eligibility_and_copies_for_the_recorded_caster() {
    crate::test_support::init_config();
    check_selection(i64::from(i32::MAX) + 1, 31270112);
    check_selection(-1, 31390121);
}

fn check_selection(caster_uid: i64, skill_id: i32) {
    let entity = |uid, model_id, group| FightEntityInfo {
        uid: Some(uid),
        model_id: Some(model_id),
        current_hp: Some(100),
        skill_group1: group,
        ..Default::default()
    };
    let source_uid = -1;
    let mut allies = vec![
        entity(source_uid, 3139, vec![31390111, 31390121]),
        entity(12, 3127, vec![skill_id]),
    ];
    if caster_uid != source_uid {
        allies.push(entity(caster_uid, 3127, vec![skill_id]));
    }
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: allies,
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let grant = managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid,
            target_uid: source_uid,
            buff_id: 235002,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    let buff_uid = grant.change.added.as_ref().unwrap().buff.uid.unwrap();
    let act_id = 1104;
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60283, "BufferflyRecordSkill"),
    };
    let mut target = TargetContext {
        recorded_skill_id: skill_id,
        recorded_skill_source_uid: caster_uid,
        ..Default::default()
    };
    let ops = record(
        BehaviorOpContext {
            source_uid,
            source_team: 1,
            target_uid: source_uid,
            active_skill_id: 31390131,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut RoundDeterminism::default(),
            modifiers: &mut SkillModifiers::default(),
            target: &mut target,
        },
        origin,
    )
    .unwrap();
    let [
        RuleOp::Command(BattleCommand::Buff(command)),
        RuleOp::BuffActInfoMarker(marker),
    ] = ops.as_slice()
    else {
        panic!("expected state commit and the buff act update");
    };
    assert_eq!(
        (marker.buff_uid, marker.target_uid, marker.act_id),
        (buff_uid, source_uid, act_id)
    );
    assert_eq!(marker.params, [1, 2, 3, 4, 5, 6, 9, 13]);
    assert_eq!(marker.str_param, Some(format!("3,{skill_id},{caster_uid}")));
    managers.execute_buff(command.clone()).unwrap();

    let subscriber = BuffActSubscriber {
        owner_uid: source_uid,
        source_uid,
        buff_uid,
        buff_id: 235002,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundStartCard,
            DefinitionKey::new(act_id, "ButterflyRecordSkill"),
        ),
        act_type: "ButterflyRecordSkill".into(),
        effect_time: 105,
        effect_condition: 0,
        args: vec![3, 10010, 1, 2, 3, 4, 5, 6, 9, 13],
        raw: "1104#3#10010#1,2,3,4,5,6,9,13".into(),
    };
    let event = BattleEvent::Kind(EventKind::RoundStartCard);
    let ops = rule_ops(&managers, &pool, &subscriber, &event).unwrap();
    let [copies @ .., RuleOp::Command(BattleCommand::Buff(reset))] = ops.as_slice() else {
        panic!("expected copies and the state reset");
    };
    assert_eq!(copies.len(), 3);
    let caster_model = pool.entity(caster_uid).unwrap().model_id;
    assert!(copies.iter().all(|op| matches!(op,
        RuleOp::Command(BattleCommand::Card(CardCommand::AddPrecast(add)))
            if add.card.uid == Some(caster_uid) && add.card.skill_id == Some(skill_id)
                && add.card.hero_id == Some(caster_model)
                && add.card.temp_card == Some(true)
                && add.card.card_type == Some(sonettobuf::card_info::CardType::None as i32)
                && matches!(add.card.enchants.as_slice(), [enchant]
                    if enchant.enchant_id == Some(10010) && enchant.duration == Some(-1))
    )));
    managers.execute_buff(reset.clone()).unwrap();
    let buff = managers.buff.snapshot(source_uid, buff_uid).unwrap();
    let info = buff
        .act_info
        .iter()
        .find(|info| info.act_id == Some(act_id))
        .unwrap();
    assert_eq!(info.param, marker.params);
    assert_eq!(info.str_param.as_deref(), Some("3,0,0"));
    assert!(
        rule_ops(&managers, &pool, &subscriber, &event)
            .unwrap()
            .is_empty()
    );
}
