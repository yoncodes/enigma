use super::*;
use sonettobuf::{CardData, card_data::CardDataKey};

#[test]
fn sovereign_card_leaves_room_for_the_dealt_ultimate() {
    let fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                model_id: Some(3147),
                current_hp: Some(100),
                ex_point: Some(5),
                ex_point_max: Some(5),
                ex_skill: Some(31473141),
                buffs: vec![BuffInfo {
                    uid: Some(20),
                    from_uid: Some(10),
                    buff_id: Some(31470010),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let sovereign = CardInfo {
        uid: Some(10),
        skill_id: Some(31472131),
        card_dataes: vec![CardData {
            key: Some(CardDataKey::Unnamed as i32),
            value: Some(r#"{"lock":false,"strengthen":{"1":4,"2":4,"3":4,"4":4}}"#.into()),
        }],
        ..Default::default()
    };

    assert!(CardMechanic.is_ultimate(&managers, &sovereign, &pool.attacker_main[0]));
    let dealt = CardMechanic.special_team_cards(&pool, &managers, std::slice::from_ref(&sovereign));
    let [ultimate] = dealt.as_slice() else {
        panic!("expected one dealt ultimate, got {dealt:?}");
    };
    assert_eq!(ultimate.skill_id, Some(31473141));
    assert!(
        CardMechanic
            .special_team_cards(&pool, &managers, &[sovereign, ultimate.clone()])
            .is_empty()
    );
}
