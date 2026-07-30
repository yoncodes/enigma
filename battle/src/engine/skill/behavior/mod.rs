pub mod action_point;
pub mod additional_damage;
pub mod attr_fix_by_burn_layer;
pub mod attr_fix_by_lost_hp;
pub mod buff;
pub mod card;
pub mod card_limit;
pub mod career;
pub mod classify;
pub mod crystal_card;
pub mod damage_target;
pub mod detonate;
pub mod electric;
pub mod gauge;
pub mod general;
pub mod injury_bank;
pub mod kill;
pub mod magic_circle;
pub mod monster_change;
pub mod nuo_di_ka;
pub mod poison;
pub mod precast;
pub mod rate;
pub mod registry;
pub mod resource;
pub mod scene;
pub mod shell;
pub mod shield;
pub mod skill_modifier;
pub mod special_count;
pub mod summon;
pub mod synchronization;
pub mod ultimate_kind;
pub mod use_skill;

use crate::engine::{
    event::payload::BattleEvent,
    manager::BattleManagers,
    runtime::determinism::RoundDeterminism,
    skill::{
        action::SkillModifiers,
        effect::ParsedBehavior,
        rule::output::RuleOp,
        target::{TargetContext, TargetPool},
    },
};

pub struct BehaviorOpContext<'a> {
    pub source_uid: i64,
    pub source_team: i32,
    pub target_uid: i64,
    pub active_skill_id: i32,
    pub transfer_count: i32,
    pub event: Option<&'a BattleEvent>,
    pub managers: &'a BattleManagers,
    pub pool: &'a TargetPool,
    pub determinism: &'a mut RoundDeterminism,
    pub modifiers: &'a mut SkillModifiers,
    pub target: &'a mut TargetContext,
}

pub struct AttackModifierContext<'a> {
    pub operation: BehaviorOpContext<'a>,
    pub conditions: &'a [crate::engine::skill::condition::ParsedCondition],
}

pub fn rule_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
    let definition = registry::find(behavior)?;
    if definition
        .supports
        .is_some_and(|supports| !supports(behavior))
    {
        return None;
    }
    (definition.emit_ops)(context, behavior)
}

pub fn is_supported(behavior: &ParsedBehavior) -> bool {
    registry::find(behavior).is_some_and(|definition| {
        definition
            .supports
            .is_none_or(|supports| supports(behavior))
    })
}

pub fn has_destination(behavior: &ParsedBehavior) -> bool {
    registry::find(behavior).is_some_and(|definition| definition.destination)
}

pub fn runs_after_row_damage(behavior: &ParsedBehavior) -> bool {
    registry::find(behavior)
        .is_some_and(|definition| definition.phase == registry::BehaviorPhase::AfterDamage)
}

pub fn routes_configured_damage(behavior: &ParsedBehavior) -> bool {
    registry::find(behavior)
        .is_some_and(|definition| definition.kind == classify::BehaviorKind::ConfiguredDamageTarget)
}

pub fn command_origin(
    behavior: &ParsedBehavior,
) -> Option<crate::engine::skill::rule::CommandOrigin> {
    registry::find(behavior).map(|definition| crate::engine::skill::rule::CommandOrigin {
        domain: crate::engine::skill::rule::RuleDomain::Behavior,
        key: definition.key,
    })
}

#[allow(clippy::too_many_arguments)]
#[cfg(test)]
mod op_tests {
    use super::*;
    use crate::engine::{
        manager::card::{CardCommand, CardEnergyChange},
        skill::rule::output::BattleCommand,
    };

    #[test]
    fn exact_definition_owns_its_destination_op_emitter() {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::new(60189, "AddEnergyToCard", vec![1, -1, 3]);

        assert!(matches!(
            rule_ops(
                BehaviorOpContext {
                    source_uid: 10,
                    source_team: 1,
                    target_uid: 10,
                    active_skill_id: 0,
                    transfer_count: 1,
                    event: None,
                    managers: &managers,
                    pool: &pool,
                    determinism: &mut determinism,
                    modifiers: &mut modifiers,
                    target: &mut target,
                },
                &behavior,
            ),
            Some(ops) if matches!(ops.as_slice(), [RuleOp::Command(BattleCommand::Card(
                CardCommand::ChangeBasicEnergy(CardEnergyChange {
                    delta: -1,
                    count: 3,
                    ..
                })
            ))])
        ));
    }

    #[test]
    fn exact_ultimate_kind_definition_emits_entity_owned_state() {
        crate::test_support::init_config();
        let fight = sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    team_type: Some(1),
                    ex_skill: Some(900),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let pool = TargetPool::from_fight(&fight);
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::new(100012, "EzioBigSkillWeapon2", Vec::new());

        assert!(matches!(
            rule_ops(
                BehaviorOpContext {
                    source_uid: 10,
                    source_team: 1,
                    target_uid: 10,
                    active_skill_id: 0,
                    transfer_count: 1,
                    event: None,
                    managers: &managers,
                    pool: &pool,
                    determinism: &mut determinism,
                    modifiers: &mut modifiers,
                    target: &mut target,
                },
                &behavior,
            ),
            Some(ops) if matches!(
                ops.as_slice(),
                [RuleOp::Command(BattleCommand::EntitySkill(command))]
                    if command.target_uid == 10
                        && command.ultimate_kind
                            == crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction
                        && command.origin.key
                            == crate::engine::skill::rule::DefinitionKey::new(
                                100012,
                                "EzioBigSkillWeapon2",
                            )
            )
        ));
        assert!(registry::find_key(100012, "EzioBigSkillWeapon2").is_some());
        assert!(registry::find_key(100012, "EzioReuse").is_none());
    }
}
