use sonettobuf::{Fight, FightEntityInfo, FightTeam, PowerInfo};

use super::*;
use crate::engine::{
    event::subscription::SubscriptionKey,
    manager::eureka::{EurekaChange, EurekaCommand},
    skill::{
        buff_act::registry::{BuffActDestination, RuntimeFrameScope, destination, find},
        rule::{CommandOrigin, DefinitionKey, RuleDomain},
    },
};

fn origin() -> CommandOrigin {
    CommandOrigin {
        domain: RuleDomain::BuffAct,
        key: DefinitionKey::new(1085, "TeamPerPowerAddBuff"),
    }
}

fn subscriber() -> BuffActSubscriber {
    BuffActSubscriber {
        owner_uid: 2,
        source_uid: 2,
        buff_uid: 50,
        buff_id: 30530205,
        team_type: 1,
        owner_alive: true,
        amount: 1,
        key: SubscriptionKey::new(
            EventKind::RoundEndEntitySettlement,
            DefinitionKey::new(1085, "TeamPerPowerAddBuff"),
        ),
        act_type: "TeamPerPowerAddBuff".to_owned(),
        effect_time: 303,
        effect_condition: 0,
        args: vec![1, 1, 4, 30530201],
        raw: "1085#1#1#4#30530201".to_owned(),
    }
}

#[test]
fn route_and_arguments_are_exact() {
    crate::test_support::init_config();
    let feature = super::super::registry::resolve_feature(
        Some(config::configs::get()),
        "1085#1#1#4#30530201",
    )
    .unwrap();

    assert!(supports(&feature.args));
    assert_eq!(feature.kind, Some(BuffActKind::TeamPerPowerAddBuff));
    assert_eq!(feature.references(None).buffs, vec![30530201]);
    assert_eq!(
        find(1085, "TeamPerPowerAddBuff")
            .unwrap()
            .runtime
            .frame_scope,
        RuntimeFrameScope::CausingFrame
    );
    assert_eq!(
        destination(1085, "TeamPerPowerAddBuff", &feature.args),
        Some(BuffActDestination::Runtime)
    );
    let wire = find(1085, "TeamPerPowerAddBuff").unwrap().wire.unwrap();
    assert_eq!(
        wire.markers(super::super::wire::WirePhase::Add),
        &[sonettobuf::effect_type_enum::EffectType::None as i32]
    );
    assert!(
        wire.markers(super::super::wire::WirePhase::Static)
            .is_empty()
    );
    assert!(
        wire.markers(super::super::wire::WirePhase::Refresh)
            .is_empty()
    );
}

#[test]
fn grants_one_layer_for_each_four_team_eureka_spent() {
    crate::test_support::init_config();
    let entities = [1, 2, 3, 4].map(|uid| FightEntityInfo {
        uid: Some(uid),
        team_type: Some(1),
        current_hp: Some(1_000),
        power_infos: vec![PowerInfo {
            power_id: Some(1),
            num: Some(10),
            max: Some(10),
        }],
        ..Default::default()
    });
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: entities.into(),
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    for (uid, spent) in [(1, 8), (2, 2), (3, 4), (4, 2)] {
        managers
            .execute_eureka(EurekaCommand::Change(EurekaChange {
                origin: origin(),
                source_uid: uid,
                target_uid: uid,
                power_id: 1,
                delta: -spent,
                effect_type: 0,
            }))
            .unwrap();
    }

    let ops = rule_ops(
        &managers,
        &pool,
        &subscriber(),
        &BattleEvent::Kind(EventKind::RoundEndEntitySettlement),
    )
    .unwrap();
    assert!(matches!(
        ops.as_slice(),
        [RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
            BuffGrant {
                source_uid: 2,
                target_uid: 2,
                buff_id: 30530201,
                amount: Some(4),
                ..
            }
        )))]
    ));
}
