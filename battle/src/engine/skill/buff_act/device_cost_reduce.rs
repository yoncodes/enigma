use crate::engine::{
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffSelector, DepletedBuff},
    },
    skill::rule::output::{BattleCommand, RuleOp},
};

use super::{feature_command_origin, is_kind, registry::BuffActKind};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [reduction] if *reduction > 0)
}

pub fn modifier(
    managers: &BattleManagers,
    source_uid: i64,
    skill_id: i32,
) -> Option<(i32, RuleOp)> {
    let (team, skill) = managers.conduit.skill(source_uid, skill_id)?;
    if skill.cost_type == 999 || skill.cost_value <= 0 {
        return None;
    }
    let feature = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .find(|feature| {
            feature.team_type == team
                && feature.amount > 0
                && is_kind(feature, BuffActKind::DeviceCostReduce)
                && supports(feature.values.get(1..).unwrap_or_default())
        })?;
    let reduction = feature.values[1].min(skill.cost_value);
    let consume = RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
        origin: feature_command_origin(&feature)?,
        target_uid: feature.owner_uid,
        selector: BuffSelector::Uid(feature.buff_uid),
        amount: 1,
        depleted: DepletedBuff::Remove,
    })));
    Some((reduction, consume))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    #[test]
    fn team_conduit_uses_one_calibration_stack_for_one_reduced_activation() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        model_id: Some(3134),
                        current_hp: Some(100),
                        buffs: vec![BuffInfo {
                            uid: Some(20),
                            buff_id: Some(31440113),
                            count: Some(2),
                            from_uid: Some(10),
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(11),
                        model_id: Some(3149),
                        current_hp: Some(100),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    model_id: Some(1001),
                    current_hp: Some(100_000),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);

        managers
            .conduit
            .execute(
                crate::engine::manager::conduit::ConduitCommand::SelectGroup {
                    source_uid: 11,
                    group: 2,
                },
            )
            .unwrap();
        let (reduction, consume) = modifier(&managers, 11, 31490141).unwrap();
        assert_eq!(reduction, 1);
        let RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(consume_command))) = consume
        else {
            panic!("expected one exact buff consumption")
        };
        assert!(matches!(
            consume_command,
            BuffConsume {
                target_uid: 10,
                selector: BuffSelector::Uid(20),
                amount: 1,
                ..
            }
        ));
        managers
            .conduit
            .execute(
                crate::engine::manager::conduit::ConduitCommand::ChangePower(
                    crate::engine::manager::conduit::ConduitPowerChange {
                        origin: consume_command.origin,
                        source_uid: 11,
                        team: 1,
                        power_id: 2,
                        delta: 1,
                        kind: crate::engine::manager::conduit::ConduitPowerChangeKind::Standard,
                    },
                ),
            )
            .unwrap();
        let pool = crate::engine::skill::target::TargetPool::from_fight(&fight);
        let mut catalog = crate::engine::skill::effect::SkillEffectCatalog::from_fight(
            config::configs::get(),
            &fight,
        );
        crate::engine::runtime::schedule::run_conduit_phase(
            &fight,
            &mut managers,
            &pool,
            &mut catalog,
            &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
            crate::engine::skill::target::TargetContext::default(),
            &[sonettobuf::FightDeviceOper {
                uid: Some(11),
                index: Some(2),
            }],
        )
        .unwrap();

        assert_eq!(managers.conduit.power(1, 2), 0);
        assert_eq!(
            managers
                .buff
                .buff_act_amount(10, BuffActKind::DeviceCostReduce),
            0
        );
    }
}
