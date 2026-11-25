use sonettobuf::{BuffInfo, EquipRecord, Fight, FightEntityInfo, FightTeam};

use super::*;

#[test]
fn equipped_target_buff_bonus_uses_each_enemys_opening_state() {
    crate::test_support::init_config();
    let burning = BuffInfo {
        uid: Some(1000),
        buff_id: Some(4150001),
        from_uid: Some(10),
        layer: Some(1),
        duration: Some(1),
        ..Default::default()
    };
    let enemy = |uid, buffs| FightEntityInfo {
        uid: Some(uid),
        current_hp: Some(100),
        buffs,
        ..Default::default()
    };
    let mut fight = Fight {
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(10),
                current_hp: Some(100),
                passive_skill: vec![432315],
                equips: vec![EquipRecord {
                    equip_id: Some(1523),
                    refine_lv: Some(5),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }),
        defender: Some(FightTeam {
            entitys: vec![enemy(-1, vec![burning.clone()]), enemy(-2, Vec::new())],
            ..Default::default()
        }),
        ..Default::default()
    };
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let snapshot = OpeningTargetBuffs::capture(10, &catalog, &managers, &pool);
    let slot = &catalog.get(432315).unwrap().slots[0];
    assert_eq!(snapshot.matches(slot, &[-1]), Some(true));
    assert_eq!(snapshot.matches(slot, &[-2]), Some(false));

    fight.defender.as_mut().unwrap().entitys[1]
        .buffs
        .push(BuffInfo {
            uid: Some(1001),
            ..burning
        });
    let managers = BattleManagers::seeded(&fight);
    assert!(managers.buff.has_active_buff_id_or_type(-2, 4150001));
    assert_eq!(snapshot.matches(slot, &[-2]), Some(false));

    let unrelated = &catalog.get(432315).unwrap().slots[1];
    assert_eq!(snapshot.matches(unrelated, &[-1]), None);
}
