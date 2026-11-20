use super::*;

#[test]
fn buff_rejection_reports_the_committed_behavior_opcode() {
    crate::test_support::init_config();
    let blocker_id = 30810321;
    let origin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(50035, "AddBuffBasedOnEnemyBurnUseCount"),
    };
    let mut managers = BattleManagers::seeded(&Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                buffs: vec![sonettobuf::BuffInfo {
                    uid: Some(100),
                    buff_id: Some(blocker_id),
                    from_uid: Some(10),
                    duration: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    });
    let changes = managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: 10,
            buff_id: 30810108,
            amount: Some(15),
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();
    assert_eq!(changes.origin, origin);

    let effects = EffectPacket::recorded_buff_changes(&changes);
    assert_eq!(effects.len(), 1);
    assert_eq!(effects[0].effect_type, Some(EffectType::Buffreject as i32));
    assert_eq!(effects[0].effect_num, Some(blocker_id));
    assert_eq!(effects[0].config_effect, Some(origin.key.opcode));
}
