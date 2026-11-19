use super::*;

fn debuff_order(card_index: i32) -> Vec<i64> {
    crate::test_support::init_config();
    let entity = |uid| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        attr: Some(HeroAttribute {
            hp: Some(100),
            ..Default::default()
        }),
        ..Default::default()
    };
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![entity(10)],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![entity(-1), entity(-2), entity(-3)],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut catalog = SkillEffectCatalog::default();
    catalog.insert(ParsedSkillEffect {
        skill_id: 100,
        slots: vec![SkillEffectSlot::new(
            ParsedBehavior::new(1, "AddBuff", vec![300401]),
            TargetRequest {
                code: 202,
                raw: Vec::new(),
            },
        )],
    });
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let mut invocation: SkillInvocation = SkillRequest {
        source_uid: 10,
        skill_id: 100,
    }
    .into();
    invocation.card_index = card_index;
    invocation.target = crate::engine::skill::action::SkillTarget::Explicit(-2);
    let result = run_skill(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        invocation,
        Default::default(),
    )
    .unwrap();
    result
        .frames
        .iter()
        .flat_map(|frame| frame.items.iter())
        .filter_map(|item| match item {
            FrameItem::Change(change) => match change.as_ref() {
                crate::engine::runtime::change::BattleChange::Buff(changes) => {
                    changes.change.added.as_ref().map(|added| added.target_uid)
                }
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn played_mass_debuff_card_starts_with_the_selected_enemy() {
    assert_eq!(debuff_order(1), [-2, -1, -3]);
    assert_eq!(debuff_order(0), [-1, -2, -3]);
}
