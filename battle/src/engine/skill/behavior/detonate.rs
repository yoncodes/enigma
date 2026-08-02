use crate::engine::{
    event::kind::EventKind,
    manager::{
        buff::{BuffCommand, BuffRemove, BuffRemoveSelector},
        hp::HpCommand,
    },
    skill::{
        behavior::{BehaviorOpContext, registry::BehaviorHandler},
        buff_act::{self, registry::BuffActKind},
        effect::ParsedBehavior,
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(super) struct Handler;

impl BehaviorHandler for Handler {
    fn emit_ops(context: BehaviorOpContext<'_>, behavior: &ParsedBehavior) -> Option<Vec<RuleOp>> {
        let selectors = behavior.arg_list(0)?;
        let include_poison = behavior.arg(1)? != 0;
        let consume = behavior.arg(2)? != 0;
        let rate = behavior.arg(3)?;
        if rate <= 0 {
            return Some(Vec::new());
        }

        let origin = super::command_origin(behavior)?;
        let features = context.managers.buff.detonation_features(
            &context.managers.hp,
            context.target_uid,
            &selectors,
            include_poison,
        );
        let mut effects = Vec::new();
        let mut consumed = Vec::new();
        for feature in features {
            let Some(subscriber) = buff_act::subscriber_from_feature(feature, EventKind::RoundEnd)
            else {
                continue;
            };
            let Some(kind) = buff_act::subscriber_kind(&subscriber) else {
                continue;
            };
            if !matches!(
                kind,
                BuffActKind::Cure | BuffActKind::AdvancedCure | BuffActKind::Poison
            ) {
                continue;
            }
            if !consumed.contains(&subscriber.buff_uid) {
                consumed.push(subscriber.buff_uid);
            }
            let command = match kind {
                BuffActKind::Cure | BuffActKind::AdvancedCure => {
                    buff_act::cure::heal_command(context.managers, &subscriber, 20009)
                }
                BuffActKind::Poison => buff_act::damage_over_time::detonation_rule_op(
                    context.managers,
                    context.pool,
                    context.determinism,
                    &subscriber,
                    consume,
                )
                .and_then(|op| match op {
                    RuleOp::Command(BattleCommand::Hp(command)) => Some(command),
                    _ => None,
                }),
                _ => None,
            };
            let duration = if consume && !matches!(kind, BuffActKind::Poison) {
                context
                    .managers
                    .buff
                    .snapshot(subscriber.owner_uid, subscriber.buff_uid)
                    .and_then(|buff| buff.duration)
                    .unwrap_or(1)
                    .max(1)
            } else {
                1
            };
            let Some(command) =
                command.and_then(|command| scaled(command, origin, rate.saturating_mul(duration)))
            else {
                continue;
            };
            effects.push(RuleOp::Command(BattleCommand::Hp(command)));
        }
        if consume {
            effects.extend(consumed.into_iter().map(|buff_uid| {
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                    origin,
                    target_uid: context.target_uid,
                    selector: BuffRemoveSelector::Uid(buff_uid),
                })))
            }));
        }
        Some(effects)
    }
}

fn scaled(
    command: HpCommand,
    origin: crate::engine::skill::rule::CommandOrigin,
    rate: i32,
) -> Option<HpCommand> {
    match command {
        HpCommand::Heal(mut heal) => {
            heal.amount = heal.amount.saturating_mul(rate) / 1000;
            heal.config_effect = 20009;
            heal.origin = origin;
            (heal.amount > 0).then_some(HpCommand::Heal(heal))
        }
        HpCommand::Lose(mut loss) => {
            loss.amount = loss.amount.saturating_mul(rate) / 1000;
            loss.config_effect = 20009;
            loss.origin = origin;
            if let Some(hurt) = &mut loss.hurt {
                hurt.display_amount = Some(loss.amount);
            }
            (loss.amount > 0).then_some(HpCommand::Lose(loss))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use super::*;
    use crate::engine::skill::{
        behavior::classify::BehaviorSpec,
        target::{TargetContext, TargetPool},
    };
    use crate::engine::{manager::BattleManagers, runtime::determinism::RoundDeterminism};

    #[test]
    fn detonate_resolves_selected_cure_then_consumes_its_instance() {
        crate::test_support::init_config();
        let managers = BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![BuffInfo {
                        uid: Some(20),
                        buff_id: Some(600101),
                        from_uid: Some(10),
                        duration: Some(2),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        });
        let behavior = ParsedBehavior::from_spec(
            BehaviorSpec::new(20009, "Detonate2"),
            vec![5003, 0, 1, 1000],
            vec!["5003".into(), "0".into(), "1".into(), "1000".into()],
        );
        let pool = TargetPool::default();
        let mut determinism = RoundDeterminism::default();
        let mut modifiers = Default::default();
        let mut target = TargetContext::default();

        let ops = Handler::emit_ops(
            BehaviorOpContext {
                source_uid: 10,
                source_team: 1,
                target_uid: 10,
                active_skill_id: 1,
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
            [
                RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(heal))),
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(BuffRemove {
                    selector: BuffRemoveSelector::Uid(20),
                    ..
                })))
            ] if heal.amount == 1_000 && heal.config_effect == 20009
        ));
    }
}
