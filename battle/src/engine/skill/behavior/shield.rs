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

type ShieldFormula = (i32, AttrId, i32, AttrId, i32, Option<(AttrId, i32)>);

fn formula(behavior: &ParsedBehavior) -> Option<ShieldFormula> {
    match behavior.args.as_slice() {
        [buff_id, raw_attr, rate, _, raw_max_attr, max_rate] => Some((
            *buff_id,
            AttrId::from_raw(*raw_attr)?,
            *rate,
            AttrId::from_raw(*raw_max_attr)?,
            *max_rate,
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
        )),
        _ => None,
    }
}

fn supports(behavior: &ParsedBehavior) -> bool {
    formula(behavior).is_some()
}

impl BehaviorHandler for Handler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        supports(behavior)
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        emit_ops(context, behavior, ShieldCarrierUid::Definition)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior)
    }
}

impl BehaviorHandler for ChildUidHandler {
    const VALIDATES_ARGUMENTS: bool = true;

    fn supports(behavior: &ParsedBehavior) -> bool {
        supports(behavior)
    }

    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        emit_ops(context, behavior, ShieldCarrierUid::Child)
    }

    fn references(behavior: &ParsedBehavior) -> RuleReferences {
        references(behavior)
    }
}

fn emit_ops(
    context: BehaviorOpContext<'_>,
    behavior: &ParsedBehavior,
    carrier_uid: ShieldCarrierUid,
) -> Option<Vec<RuleOp>> {
    let (buff_id, amount_attr, rate, max_attr, max_rate, multiplier_bonus) = formula(behavior)?;
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

fn references(behavior: &ParsedBehavior) -> RuleReferences {
    RuleReferences {
        buffs: behavior.arg(0).into_iter().collect(),
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
