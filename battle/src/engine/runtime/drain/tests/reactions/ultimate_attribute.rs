use super::*;

#[test]
fn ultimate_attributes_reach_preselection_and_settlement() {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100_000),
        attr: Some(HeroAttribute {
            hp: Some(100_000),
            attack: Some(1_000),
            ..Default::default()
        }),
        ..Default::default()
    };
    for (skill_id, damage) in [(30620111, 1_000), (30620137, 2_008)] {
        let mut source = entity(10);
        source.buffs.push(BuffInfo {
            uid: Some(20),
            buff_id: Some(30620121),
            layer: Some(2),
            from_uid: Some(10),
            ..Default::default()
        });
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![source],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![entity(-1)],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        managers.attribute.override_ex(
            10,
            &HeroExAttribute {
                cri: Some(900),
                cri_dmg: Some(1_500),
                ..Default::default()
            },
        );
        let mut catalog = SkillEffectCatalog::from_game_db(config::configs::get());
        catalog.insert_alias(skill_id, skill_id);
        catalog.insert(ParsedSkillEffect {
            skill_id,
            slots: Vec::new(),
        });
        catalog.insert_damage_rate(skill_id, 1_000);
        let mut invocation: SkillInvocation = SkillRequest {
            source_uid: 10,
            skill_id,
        }
        .into();
        invocation.phase = Some(crate::engine::skill::action::SkillPhase::Damage);
        invocation.target = SkillTarget::Explicit(-1);
        let seed = (0..1000)
            .find(|seed| !RoundDeterminism::with_seed(*seed).roll_crit(skill_id, 10, -1, 900))
            .unwrap();
        run(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::with_seed(seed),
            TargetContext::default(),
            [RuleOp::Skill(invocation)],
        )
        .unwrap();
        assert_eq!(managers.hp.current(-1), 100_000 - damage);
        assert_eq!(managers.buff.buff_id_amount(10, 30620121), 2);
        assert_eq!(managers.origin_attribute(10, AttrId::CriticalRate), 900);
        assert_eq!(managers.origin_attribute(10, AttrId::CriticalDmg), 1_500);
    }
}
