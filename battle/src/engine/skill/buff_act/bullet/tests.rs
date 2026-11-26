use crate::engine::event::{kind::EventKind, subscription::SubscriptionKey};

use super::*;

#[test]
fn skill_cast_publishes_the_feature_without_owning_carrier_consumption() {
    let mut subscriber = BuffActSubscriber {
        owner_uid: 10,
        source_uid: 10,
        buff_uid: 20,
        buff_id: 30,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::SkillCast,
            crate::engine::skill::rule::DefinitionKey::new(827, "Bullet"),
        ),
        act_type: "Bullet".to_owned(),
        effect_time: 208,
        effect_condition: 3,
        args: Vec::new(),
        raw: "827".to_owned(),
    };

    let definition = super::super::registry::find(827, "Bullet").unwrap();
    assert_eq!(
        definition.runtime.frame_scope,
        super::super::registry::RuntimeFrameScope::SubscriberFrame
    );
    assert_eq!(
        definition.runtime.frame_source,
        super::super::registry::RuntimeFrameSource::Applier
    );
    let event = BattleEvent::SkillAction(attack());
    assert!(matches!(
        rule_ops(&subscriber, &event).as_deref(),
        Some([RuleOp::Publish(BattleEvent::BuffFeatureTriggered(trigger))])
            if trigger.owner_uid == 10
                && trigger.target_uid == -1
                && trigger.buff_uid == 20
                && trigger.act_id == 827
    ));
    subscriber.owner_alive = false;
    assert_eq!(rule_ops(&subscriber, &event), Some(Vec::new()));
}

fn attack() -> crate::engine::skill::action::SkillActionEvent {
    crate::engine::skill::action::SkillActionEvent {
        source_uid: 10,
        card_index: 0,
        target_uid: -1,
        skill_id: 100,
        is_attack: true,
        target_uids: vec![-1],
        attacked_target_uids: vec![-1],
        phase: crate::engine::skill::action::SkillPhase::HitPassives,
        skill_slot: 0,
        rank: 1,
        skill_type: 0,
        effect_tag: 1,
        assassinate: false,
        ignore_riposte: false,
        damage_amount: 1,
        kill_count: 0,
        crit_count: 0,
        guard_break_count: 0,
        additional_moxie: 0,
        extra_skill_kind: 0,
        mode: crate::engine::skill::action::SkillExecutionMode::Active,
        teammate_injury_count: 0,
        teammate_injury_count_not_reset: 0,
        team_injury_count_round: 0,
        card_enchants: Vec::new(),
        buff_additions: Vec::new(),
    }
}

#[test]
fn a_counted_target_bullet_spends_when_no_target_receives_the_grant() {
    use crate::engine::{
        manager::BattleManagers,
        runtime::determinism::RoundDeterminism,
        skill::{effect::SkillEffectCatalog, subscriber::for_active_buffs, target::TargetPool},
    };
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                team_type: Some(1),
                current_hp: Some(1_000),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    buff_id: Some(30920151),
                    from_uid: Some(10),
                    count: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let pool = TargetPool::from_fight(&fight);
    let mut determinism = RoundDeterminism::default();
    let grant = for_active_buffs(&managers, EventKind::SkillCast)
        .into_iter()
        .find(|buff| buff.act_type == "AddToTarget")
        .unwrap();
    assert!(matches!(
        recipients(&managers, &grant, true),
        RecipientScope::Primary
    ));
    for (buff_id, debuff_id) in [(722501, 722503), (8112651, 4011)] {
        let compound = BuffActSubscriber {
            buff_id,
            args: vec![1, debuff_id],
            ..grant.clone()
        };
        assert!(matches!(
            recipients(&managers, &compound, true),
            RecipientScope::Action
        ));
    }
    let event = BattleEvent::SkillAction(crate::engine::skill::action::SkillActionEvent {
        target_uid: 0,
        target_uids: Vec::new(),
        ..attack()
    });
    let ops = add_to_target(&mut RuntimeContext {
        managers: &managers,
        catalog: &catalog,
        pool: &pool,
        determinism: &mut determinism,
        subscriber: &grant,
        event: Some(&event),
    })
    .unwrap();
    let [op] = ops.as_slice() else {
        panic!("expected one charge trigger and no grant");
    };
    let RuleOp::Publish(trigger) = &op.op else {
        panic!("expected a charge trigger");
    };
    let bullet = for_active_buffs(&managers, EventKind::BuffFeatureTriggered)
        .into_iter()
        .find(|buff| buff.act_type == "Bullet")
        .unwrap();
    let ops = reaction(&mut RuntimeContext {
        managers: &managers,
        catalog: &catalog,
        pool: &pool,
        determinism: &mut determinism,
        subscriber: &bullet,
        event: Some(trigger),
    })
    .unwrap();
    assert!(matches!(
        ops.as_slice(),
        [BuffActRuleOp {
            op: RuleOp::Command(BattleCommand::Buff(BuffCommand::ConsumeAttackCharge(
                BuffConsume {
                    selector: BuffSelector::Uid(20),
                    amount: 1,
                    ..
                }
            ))),
            ..
        }]
    ));
}
