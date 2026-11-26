use super::*;
use crate::engine::{
    manager::BattleManagers,
    runtime::{determinism::RoundDeterminism, drain},
    skill::{
        action::SkillModifiers,
        behavior::classify::BehaviorSpec,
        effect::SkillEffectCatalog,
        target::{TargetContext, TargetPool},
    },
};
use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

#[test]
fn arsenal_grants_the_configured_stacks_and_draws_unheld_types_first() {
    crate::test_support::init_config();
    let data = config::configs::get();
    let catalog = BattleCatalog::new(data);
    assert!(configured(catalog, 31030121).is_none());
    assert!(configured(catalog, 91130030).is_none());
    let source_uid = 10;
    let mut drafts = configured(catalog, 30920142).unwrap();
    let unheld = drafts.remove(2);
    let held = drafts[0].id;
    let fight = Fight {
        version: Some(7),
        attacker: Some(FightTeam {
            entitys: vec![FightEntityInfo {
                uid: Some(source_uid),
                team_type: Some(1),
                position: Some(1),
                current_hp: Some(100),
                buffs: drafts
                    .iter()
                    .enumerate()
                    .map(|(index, bullet)| BuffInfo {
                        uid: Some(100 + index as i64),
                        buff_id: Some(bullet.id),
                        from_uid: Some(source_uid),
                        count: Some(2),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }],
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut managers = BattleManagers::seeded(&fight);
    let pool = TargetPool::from_fight(&fight);
    let effects = SkillEffectCatalog::from_fight(data, &fight);
    let mut determinism = RoundDeterminism::default();
    determinism.enqueue_random_buffs([held]);
    let behavior = ParsedBehavior::from_spec(
        BehaviorSpec::new(20022, "AddBuffRanTypeId"),
        vec![30920142, 2],
        Vec::new(),
    );
    let ops = crate::engine::skill::behavior::rule_ops(
        BehaviorOpContext {
            source_uid,
            source_team: 1,
            target_uid: source_uid,
            active_skill_id: 30920142,
            transfer_count: 1,
            event: None,
            managers: &managers,
            pool: &pool,
            determinism: &mut determinism,
            modifiers: &mut SkillModifiers::default(),
            target: &mut TargetContext::default(),
        },
        &behavior,
    )
    .unwrap();
    let chosen = ops
        .iter()
        .filter_map(|op| match op {
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant))) => {
                Some((grant.buff_id, grant.occurrences))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(chosen, [(unheld.id, 5), (held, 5)]);
    drain::run_command_group(
        &mut managers,
        &pool,
        &effects,
        &mut determinism,
        TargetContext::default(),
        ops,
    )
    .unwrap();
    assert_eq!(managers.buff.buff_id_amount(source_uid, unheld.id), 5);
    assert_eq!(managers.buff.buff_id_amount(source_uid, held), 7);
}
