use crate::engine::{
    manager::upgrade::{UpgradeCommand, UpgradeOperation},
    skill::{
        behavior::{BehaviorOpContext, classify::BehaviorKind, registry::BehaviorHandler},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub fn rule_op(target_uid: i64, behavior: &ParsedBehavior) -> Option<RuleOp> {
    if behavior.spec.kind == BehaviorKind::ClientEffect {
        return Some(RuleOp::EffectMarker {
            target_uid,
            effect_type: sonettobuf::effect_type_enum::EffectType::Clienteffect as i32,
            effect_num: behavior.arg(0)?,
            config_effect: behavior.spec.key.opcode,
            reserve_id: None,
            reserve_str: None,
        });
    }
    if behavior.spec.kind != BehaviorKind::NotifyUpgradeHero {
        return None;
    }
    Some(RuleOp::Command(BattleCommand::Upgrade(UpgradeCommand {
        owner_uid: target_uid,
        operation: UpgradeOperation::Offer {
            origin: super::command_origin(behavior)?,
            upgrade_id: behavior.arg(0)?,
        },
    })))
}

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        rule_op(context.target_uid, behavior).map(|op| vec![op])
    }
}

pub(super) struct AssassinateHandler;

impl BehaviorHandler for AssassinateHandler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        if behavior.args.is_empty() {
            context.target.active_skill_assassinate = true;
            Some(Vec::new())
        } else {
            None
        }
    }
}

pub(super) struct DamageRateMarkerHandler;

impl BehaviorHandler for DamageRateMarkerHandler {
    fn emit_ops(_: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        (behavior.args.is_empty()
            && behavior.spec.kind == BehaviorKind::IgnoreSkillConfigDamageRate)
            .then(Vec::new)
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
            behavior::{self, classify::BehaviorSpec},
            target::{TargetContext, TargetPool},
        },
    };

    #[test]
    fn notification_offers_the_configured_upgrade() {
        let behavior = ParsedBehavior::new(60037, "NotifyUpgradeHero", vec![308665]);

        assert!(matches!(
            rule_op(10, &behavior),
            Some(RuleOp::Command(BattleCommand::Upgrade(UpgradeCommand {
                owner_uid: 10,
                operation: UpgradeOperation::Offer {
                    upgrade_id: 308665,
                    ..
                },
                ..
            })))
        ));
    }

    #[test]
    fn assassinate_marks_the_active_skill_without_emitting_a_packet_command() {
        let managers = BattleManagers::default();
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = SkillModifiers::default();
        let mut target = TargetContext::default();
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(100005, "Assassinate"),
            Vec::new(),
            Vec::new(),
        );

        let ops = behavior::rule_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 31220131,
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

        assert!(ops.is_empty());
        assert!(target.active_skill_assassinate);
    }

    #[test]
    fn ezio_damage_rate_marker_keeps_its_exact_opcode() {
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(100017, "IgnoreSkillConfigDamageRate"),
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(
            behavior.spec.kind,
            BehaviorKind::IgnoreSkillConfigDamageRate
        );
        assert_eq!(
            behavior::registry::find(&behavior).unwrap().key.opcode,
            100017
        );
    }
}
