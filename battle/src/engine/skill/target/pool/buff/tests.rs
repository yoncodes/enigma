use super::*;

#[test]
fn target_buff_metadata_uses_configured_type_and_exact_feature_identities() {
    crate::test_support::init_config();
    let catalog = crate::catalog::BattleCatalog::new(crate::test_support::game_data());
    let buff = TargetBuff::from_buff_info(
        catalog,
        &BuffInfo {
            buff_id: Some(31320113),
            from_uid: Some(42),
            ..Default::default()
        },
    );

    assert_eq!(buff.type_id, 31320113);
    assert_eq!(
        buff.status(),
        Some(crate::engine::manager::buff::BuffStatus::NegativeStatus)
    );
    assert_eq!(buff.source_uid, 42);
    assert_eq!(
        buff.definition.features,
        ["1037#104#0#2#-10#50", "1038#31320111#1#2#1", "720#3132#0",]
    );
    assert!(
        buff.has_buff_act_kind(crate::engine::skill::buff_act::registry::BuffActKind::MonsterLabel)
    );
    assert!(buff.has_monster_label(3132));
    assert!(!buff.has_monster_label(0));

    let wire_type = TargetBuff::from_buff_info(
        catalog,
        &BuffInfo {
            buff_id: Some(31320113),
            r#type: Some(77),
            ..Default::default()
        },
    );
    assert_eq!(wire_type.type_id, 77);
    assert_eq!(wire_type.source_uid, 0);
    assert!(Arc::ptr_eq(&buff.definition, &wire_type.definition));

    let mut alternate = TargetBuffDefinition::new(catalog, 31320113);
    alternate.type_id = 123;
    assert_eq!(
        wire_type,
        TargetBuff {
            definition: Arc::new(alternate),
            ..wire_type.clone()
        }
    );

    let missing = TargetBuff::from_buff_info(
        catalog,
        &BuffInfo {
            buff_id: Some(-1),
            ..Default::default()
        },
    );
    assert_eq!(missing.type_id, 0);
    assert_eq!(missing.status(), None);
    assert!(missing.definition.act_kinds.is_empty());
    assert!(missing.definition.monster_labels.is_empty());
}
