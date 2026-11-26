use super::*;
use crate::engine::{
    fight::rules::ATTACKER_SIDE_UID,
    manager::buff::{BuffCommand, BuffGrant},
};

#[test]
fn stage_master_halo_follows_the_next_wave_entrants() {
    init_config();
    let mut fight = Fight {
        battle_id: Some(120384600),
        episode_id: Some(2600101),
        version: Some(7),
        cur_wave: Some(1),
        attacker: Some(FightTeam::default()),
        defender: Some(FightTeam::default()),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    managers.advance_wave(&mut fight, 3).unwrap().unwrap();
    managers
        .execute_buff(BuffCommand::Grant(BuffGrant {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(1, "AddBuff"),
            },
            source_uid: ATTACKER_SIDE_UID,
            target_uid: ATTACKER_SIDE_UID,
            buff_id: 9260101,
            amount: None,
            occurrences: 1,
            child_uid_reservations: 0,
        }))
        .unwrap();

    let wave = managers.advance_wave(&mut fight, 4).unwrap().unwrap();
    assert_eq!(wave.entering_uids.len(), 4);
    assert!(
        wave.entering_uids
            .iter()
            .all(|&uid| !managers.buff.has_buff_id(uid, 9260103))
    );
    let pool = TargetPool::from_fight(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    run_wave_entry_setup(
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        &wave.entering_uids,
    )
    .unwrap();

    for uid in wave.entering_uids {
        let defender = managers.entity_snapshot(uid).unwrap();
        assert!(defender.buffs.iter().any(|buff| {
            buff.buff_id == Some(9260103) && buff.from_uid == Some(ATTACKER_SIDE_UID)
        }));
    }
}
