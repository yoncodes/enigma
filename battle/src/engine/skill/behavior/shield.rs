use crate::engine::{
    entity::attr::AttrId,
    manager::shield::{ShieldCarrierUid, ShieldCommand, ShieldScope},
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::{
            RuleReferences,
            output::{BattleCommand, RuleOp},
        },
    },
};

pub(super) struct Handler;
pub(super) struct ChildUidHandler;

type ShieldFormula = (
    i32,
    AttrId,
    i32,
    AttrId,
    i32,
    Option<(AttrId, i32)>,
    Option<(i32, i32)>,
);

fn formula(behavior: &ParsedBehavior, stack_bonus: bool) -> Option<ShieldFormula> {
    match behavior.args.as_slice() {
        [buff_id, raw_attr, rate, _, raw_max_attr, max_rate] => Some((
            *buff_id,
            AttrId::from_raw(*raw_attr)?,
            *rate,
            AttrId::from_raw(*raw_max_attr)?,
            *max_rate,
            None,
            None,
        )),
        [
            buff_id,
            raw_attr,
            rate,
            _,
            raw_max_attr,
            max_rate,
            raw_bonus_attr,
            bonus_rate,
        ] => Some((
            *buff_id,
            AttrId::from_raw(*raw_attr)?,
            *rate,
            AttrId::from_raw(*raw_max_attr)?,
            *max_rate,
            Some((AttrId::from_raw(*raw_bonus_attr)?, *bonus_rate)),
            None,
        )),
        [
            buff_id,
            raw_attr,
            rate,
            0,
            raw_max_attr,
            max_rate,
            raw_bonus_attr,
            bonus_rate,
            stack_buff_id,
            stack_rate,
            -1,
        ] if stack_bonus && *stack_buff_id > 0 && *stack_rate > 0 => Some((
            *buff_id,
            AttrId::from_raw(*raw_attr)?,
            *rate,
            AttrId::from_raw(*raw_max_attr)?,
            *max_rate,
            Some((AttrId::from_raw(*raw_bonus_attr)?, *bonus_rate)),
            Some((*stack_buff_id, *stack_rate)),
        )),
        _ => None,
    }
}

fn supports(behavior: &ParsedBehavior) -> bool {
    formula(behavior, false).is_some()
}

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        supports(behavior)
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        emit_ops(context, behavior, ShieldCarrierUid::Definition, false)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior, false)
    }
}

impl BehaviorHandler for ChildUidHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        formula(behavior, true).is_some()
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        emit_ops(context, behavior, ShieldCarrierUid::Child, true)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior, true)
    }
}

fn emit_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
    carrier_uid: ShieldCarrierUid,
    stack_bonus: bool,
) -> Option<Vec<RuleOp>> {
    let (buff_id, amount_attr, rate, max_attr, max_rate, multiplier_bonus, stack_bonus) =
        formula(behavior, stack_bonus)?;
    let rate = stack_bonus.map_or(rate, |(buff_id, per_stack)| {
        rate.saturating_add(
            context
                .managers
                .buff
                .buff_id_amount(context.source_uid, buff_id)
                .saturating_mul(per_stack),
        )
    });
    Some(vec![RuleOp::Command(BattleCommand::Shield(
        ShieldCommand {
            origin: super::command_origin(behavior)?,
            source_uid: context.source_uid,
            target_uid: context.target_uid,
            buff_id,
            amount_attr,
            amount_rate: rate,
            multiplier_bonus,
            max_attr,
            max_rate,
            scope: match behavior.spec.kind {
                BehaviorKind::SupplyShield2 => ShieldScope::Entity,
                BehaviorKind::SupplyTeamShareShield => ShieldScope::TeamShared,
                _ => return None,
            },
            carrier_uid,
        },
    ))])
}

fn references(behavior: &ParsedBehavior, stack_bonus: bool) -> RuleReferences {
    let mut buffs = behavior.arg(0).into_iter().collect::<Vec<_>>();
    if let Some((_, _, _, _, _, _, Some((buff_id, _)))) = formula(behavior, stack_bonus) {
        buffs.push(buff_id);
    }
    RuleReferences {
        buffs,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        manager::BattleManagers,
        runtime::determinism::RoundDeterminism,
        skill::{
            action::SkillModifiers,
            target::{TargetContext, TargetPool},
        },
    };

    fn registered_carrier_uid(behavior: &ParsedBehavior) -> ShieldCarrierUid {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let ops = super::super::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 11,
                active_skill_id: 1,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            behavior,
        )
        .unwrap();
        match ops.as_slice() {
            [RuleOp::Command(BattleCommand::Shield(command))] => command.carrier_uid,
            _ => panic!("expected one shield command"),
        }
    }

    #[test]
    fn exact_supply_shield_routes_keep_distinct_carrier_uid_policies() {
        let kiperina = ParsedBehavior::new(
            60183,
            "SupplyShield2",
            vec![31170002, 102, 1500, 0, 102, 6500],
        );
        let marsha = ParsedBehavior::new(
            60259,
            "SupplyShield2",
            vec![31270012, 102, 1800, 0, 102, 6500, 201, 900],
        );

        assert_eq!(registered_carrier_uid(&kiperina), ShieldCarrierUid::Child);
        assert_eq!(
            registered_carrier_uid(&marsha),
            ShieldCarrierUid::Definition
        );
    }

    #[test]
    fn supply_shield_uses_its_configured_buff_stack_rate() {
        crate::test_support::init_config();
        let managers = BattleManagers::seeded(&sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![
                    sonettobuf::FightEntityInfo {
                        uid: Some(10),
                        current_hp: Some(1_000),
                        attr: Some(sonettobuf::HeroAttribute {
                            hp: Some(1_000),
                            attack: Some(2_028),
                            ..Default::default()
                        }),
                        buffs: vec![sonettobuf::BuffInfo {
                            uid: Some(20),
                            buff_id: Some(4_150_001),
                            from_uid: Some(10),
                            layer: Some(3),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    sonettobuf::FightEntityInfo {
                        uid: Some(11),
                        current_hp: Some(1_000),
                        attr: Some(sonettobuf::HeroAttribute {
                            hp: Some(1_000),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        });
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::new(
            60183,
            "SupplyShield2",
            vec![
                30_941_121, 102, 1_500, 0, 102, 6_000, 102, 0, 4_150_001, 50, -1,
            ],
        );

        let ops = super::super::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 11,
                active_skill_id: 30_941_124,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Shield(command))]
                if command.amount_rate == 1_650
                    && command.max_rate == 6_000
                    && command.multiplier_bonus == Some((AttrId::Attack, 0))
        ));
        assert_eq!(
            ChildUidHandler::references(&behavior).buffs,
            [30_941_121, 4_150_001]
        );
        assert!(ChildUidHandler::supports(&behavior));
        assert!(!Handler::supports(&behavior));
        assert!(!ChildUidHandler::supports(&ParsedBehavior::new(
            60183,
            "SupplyShield2",
            vec![
                30_941_121, 102, 1_500, 1, 102, 6_000, 102, 0, 4_150_001, 50, -1,
            ],
        )));
        assert!(!ChildUidHandler::supports(&ParsedBehavior::new(
            60183,
            "SupplyShield2",
            vec![
                30_941_121, 102, 1_500, 0, 102, 6_000, 102, 0, 4_150_001, 50, 1,
            ],
        )));
    }

    #[test]
    fn marsha_shield_emits_its_critical_rate_bonus() {
        let behavior = ParsedBehavior::new(
            60259,
            "SupplyShield2",
            vec![31270012, 102, 1800, 0, 102, 6500, 201, 900],
        );
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 11,
                active_skill_id: 31270124,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(
            matches!(ops.as_slice(), [RuleOp::Command(BattleCommand::Shield(command))]
            if command.multiplier_bonus == Some((AttrId::CriticalRate, 900)))
        );
    }

    #[test]
    fn team_share_behavior_keeps_its_exact_shield_scope() {
        let behavior = ParsedBehavior::new(
            60290,
            "SupplyTeamShareShield",
            vec![31430144, 102, 2800, 0, 102, 12500],
        );
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31430144,
                transfer_count: 1,
                event: None,
                managers: &managers,
                pool: &pool,
                determinism: &mut determinism,
                modifiers: &mut modifiers,
                target: &mut target,
            },
            &behavior,
        )
        .unwrap();

        assert!(
            matches!(ops.as_slice(), [RuleOp::Command(BattleCommand::Shield(command))]
                if command.scope == ShieldScope::TeamShared)
        );
        assert_eq!(
            super::super::registry::find(&behavior)
                .unwrap()
                .output_owner,
            super::super::registry::OutputOwner::SetupParent
        );
    }
}
