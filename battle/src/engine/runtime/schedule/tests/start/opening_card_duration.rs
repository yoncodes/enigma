use super::*;
use crate::engine::{event::payload::BuffExpiry, manager::card::CardChangeKind};

#[test]
fn preparation_expires_after_opening_sovereign_unlock() {
    init_config();
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![
                FightEntityInfo {
                    uid: Some(10),
                    model_id: Some(3114),
                    entity_type: Some(1),
                    team_type: Some(1),
                    current_hp: Some(1000),
                    passive_skill: vec![311401441],
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(31130111),
                        from_uid: Some(10),
                        duration: Some(1),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
                FightEntityInfo {
                    uid: Some(11),
                    model_id: Some(3147),
                    entity_type: Some(1),
                    team_type: Some(1),
                    current_hp: Some(1000),
                    passive_skill: vec![31471152, 31470153, 31471154, 31472151],
                    ex_skill: Some(31473141),
                    ..Default::default()
                },
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    let pool = TargetPool::from_fight(&fight);
    let mut managers = BattleManagers::seeded(&fight);
    let catalog = SkillEffectCatalog::from_fight(config::configs::get(), &fight);
    let (start, _) = run_start(
        managers.catalog(),
        &mut managers,
        &pool,
        &catalog,
        &mut RoundDeterminism::default(),
        TargetContext::default(),
        CardSetup {
            hand: vec![CardInfo {
                uid: Some(10),
                skill_id: Some(31140111),
                ..Default::default()
            }],
            draw_pile: Vec::new(),
            deck_num: 0,
        },
        1,
    )
    .unwrap();

    let unlock = start
        .outcomes
        .iter()
        .position(|outcome| {
            matches!(outcome, RuleOutcome::Card(change)
                if change.kind == CardChangeKind::SovereignUnlocked)
        })
        .expect("Sovereign card unlocked");
    let (expiry, removal) = start
        .outcomes
        .iter()
        .enumerate()
        .find_map(|(index, outcome)| {
            let changes = match outcome {
                RuleOutcome::Buff(change) => std::slice::from_ref(change.as_ref()),
                RuleOutcome::BuffBatch(changes) => changes.as_slice(),
                _ => return None,
            };
            changes
                .iter()
                .flat_map(|change| &change.change.removed)
                .find(|remove| remove.buff.buff_id == Some(311401431))
                .map(|remove| (index, remove))
        })
        .expect("Preparation expired");
    assert!(unlock < expiry);
    assert_eq!(removal.expiry, BuffExpiry::Duration(106));
    assert_eq!(removal.buff.duration, Some(0));
    assert_eq!(
        managers
            .buff
            .active_for(10)
            .find(|buff| buff.buff_id == Some(31130111))
            .and_then(|buff| buff.duration),
        Some(1)
    );
    assert!(
        managers
            .card
            .hand()
            .iter()
            .any(|card| card.skill_id == Some(311401511))
    );
}
