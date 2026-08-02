use super::*;

#[test]
fn lost_life_attack_uses_all_characters_and_the_configured_hp_basis() {
    crate::test_support::init_config();
    assert!(config::get().skill_buff.get(31200174).is_some());
    let entity = |uid, current_hp| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(current_hp),
        attr: Some(HeroAttribute {
            hp: Some(10_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![entity(1, 10_000), entity(2, 5_000)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1, 8_000)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let configured = BuffManager::configured_features(31200174);
    assert!(
        configured.iter().any(|feature| {
            let mut feature = feature.clone();
            feature.owner_uid = 1;
            is_kind(&feature, BuffActKind::AttrOnlyCalDamageReplaceAttr)
                && crate::engine::skill::buff_act::attack_replacement(&feature, &managers.hp)
                    == Some(2_000)
        }),
        "configured={configured:?}"
    );
    let mut determinism = crate::engine::runtime::determinism::RoundDeterminism::default();
    let mut modifiers = crate::engine::skill::action::SkillModifiers::default();
    let mut target = TargetContext::default();
    let losses = crate::engine::skill::behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 1,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 1,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &ParsedBehavior::from_spec(
            BehaviorSpec::new(60212, "LostAllLifeByAttr"),
            vec![AttrId::CurrentHp as i32, 200, AttrId::Hp as i32, 200],
            Vec::new(),
        ),
    )
    .unwrap();
    let amounts = losses
        .iter()
        .filter_map(|op| match op {
            RuleOp::Command(BattleCommand::Hp(HpCommand::Lose(loss))) => Some(loss.amount),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(amounts, vec![2_000, 1_000, 1_600]);

    let damage = crate::engine::skill::behavior::rule_ops(
        BehaviorOpContext {
            source_uid: 1,
            source_team: 1,
            target_uid: -1,
            active_skill_id: 1,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut modifiers,
            target: &mut target,
        },
        &ParsedBehavior::from_spec(
            BehaviorSpec::new(60216, "DamageRealLostLife"),
            vec![31200174, 3, 500],
            Vec::new(),
        ),
    )
    .unwrap();
    assert!(matches!(
        damage.as_slice(),
        [RuleOp::Command(BattleCommand::Hp(HpCommand::Damage(
            HpDamage {
                amount: 3_000,
                target_uid: -1,
                ..
            }
        )))]
    ));
}

#[test]
fn configured_hp_loss_floor_clamps_loss_at_fifteen_percent() {
    crate::test_support::init_config();
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(1),
                current_hp: Some(2_000),
                attr: Some(HeroAttribute {
                    hp: Some(10_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    buff_id: Some(31200145),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(30006, "LostLifeByAttr"),
        vec![1, AttrId::CurrentHp as i32, 1_000],
        Vec::new(),
    );

    assert_eq!(managers.buff.lost_life_floor_permille(1), 150);
    assert_eq!(loss::amount(1, &managers, &behavior), Some(500));
}
