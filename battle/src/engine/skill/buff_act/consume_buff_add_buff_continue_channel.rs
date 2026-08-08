use sonettobuf::BuffActInfo;

use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{
        buff::{
            BuffActInfoMarkerResult, BuffCommand, BuffConsume, BuffRemove, BuffRemoveSelector,
            BuffSelector, BuffSetState, DepletedBuff,
        },
        BattleManagers,
    },
    skill::{
        action::SkillInvocation,
        buff_act::registry::BuffActKind,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [skill_id, resource_buff_id, 0, 50]
        if *skill_id > 0 && *resource_buff_id > 0)
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::ConsumeBuffAddBuffContinueChannel)
        || event.kind() != EventKind::RoundEnd
        || !supports(&subscriber.args)
    {
        return None;
    }
    let [_, resource_buff_id, initial_cost, cost_step] = subscriber.args.as_slice() else {
        return None;
    };
    let origin = super::command_origin(subscriber)?;
    let channel = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let current_cost = channel
        .act_info
        .iter()
        .find(|info| info.act_id == Some(subscriber.key.definition.opcode))
        .and_then(|info| info.str_param.as_deref())
        .map(str::parse::<i32>)
        .transpose()
        .ok()?
        .unwrap_or(*initial_cost);
    if current_cost < 0 {
        return None;
    }
    let available = managers
        .buff
        .buff_id_or_type_amount(subscriber.owner_uid, *resource_buff_id);
    if available < current_cost {
        return Some(vec![RuleOp::Command(BattleCommand::Buff(
            BuffCommand::Remove(BuffRemove {
                origin,
                target_uid: subscriber.owner_uid,
                selector: BuffRemoveSelector::Uid(subscriber.buff_uid),
            }),
        ))]);
    }

    let next_cost = current_cost.saturating_add(*cost_step);
    let mut act_info = channel.act_info;
    if let Some(info) = act_info
        .iter_mut()
        .find(|info| info.act_id == Some(subscriber.key.definition.opcode))
    {
        info.str_param = Some(next_cost.to_string());
    } else {
        act_info.push(BuffActInfo {
            act_id: Some(subscriber.key.definition.opcode),
            param: Vec::new(),
            str_param: Some(next_cost.to_string()),
        });
    }

    Some(vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: subscriber.owner_uid,
            selector: BuffSelector::IdOrType(*resource_buff_id),
            amount: current_cost,
            depleted: DepletedBuff::Remove,
        }))),
        RuleOp::Skill(SkillInvocation::from(super::use_skill::linked(subscriber)?)),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
            BuffSetState {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                ex_info: None,
                params: None,
                act_info: Some(act_info),
            },
        ))),
        RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            act_id: subscriber.key.definition.opcode,
            params: Vec::new(),
            str_param: Some(next_cost.to_string()),
            team_type: 0,
        }),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::skill::subscriber;
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    fn fight(resource: i32, current_cost: i32) -> Fight {
        crate::test_support::init_config();
        Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(1_000),
                    buffs: vec![
                        BuffInfo {
                            buff_id: Some(31280113),
                            uid: Some(1),
                            layer: Some(resource),
                            ..Default::default()
                        },
                        BuffInfo {
                            buff_id: Some(31280115),
                            uid: Some(2),
                            act_info: vec![BuffActInfo {
                                act_id: Some(1031),
                                param: Vec::new(),
                                str_param: Some(current_cost.to_string()),
                            }],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(1_000),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn managers(resource: i32, current_cost: i32) -> BattleManagers {
        BattleManagers::seeded(&fight(resource, current_cost))
    }

    fn subscriber(managers: &BattleManagers) -> BuffActSubscriber {
        subscriber::for_active_buffs(managers, EventKind::RoundEnd)
            .into_iter()
            .find(|subscriber| {
                super::super::subscriber_is_kind(
                    subscriber,
                    BuffActKind::ConsumeBuffAddBuffContinueChannel,
                )
            })
            .unwrap()
    }

    #[test]
    fn round_end_consumes_current_cost_casts_then_records_the_next_cost() {
        let managers = managers(110, 50);
        let subscriber = subscriber(&managers);
        let ops = rule_ops(
            &managers,
            &subscriber,
            &BattleEvent::Kind(EventKind::RoundEnd),
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
                    amount: 50,
                    ..
                }))),
                RuleOp::Skill(SkillInvocation { plan, .. }),
                RuleOp::Command(BattleCommand::Buff(BuffCommand::SetInternalState(
                    BuffSetState { act_info: Some(state), .. }
                ))),
                RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
                    str_param: Some(marker),
                    ..
                })
            ] if plan.skill_id == 31280151
                && state[0].str_param.as_deref() == Some("100")
                && marker == "100"
        ));
    }

    #[test]
    fn channel_grant_embeds_zero_counter_without_a_separate_marker() {
        let mut managers = managers(0, 0);
        let applied = managers
            .buff
            .add(&managers.hp, 10, 10, 31280115, 0)
            .unwrap();

        assert!(applied.pre_markers.is_empty());
        assert!(matches!(
            applied.buff.act_info.as_slice(),
            [BuffActInfo {
                act_id: Some(1031),
                param,
                str_param: Some(value),
            }] if param.is_empty() && value == "0"
        ));
    }

    #[test]
    fn insufficient_resource_ends_the_channel_without_casting() {
        let managers = managers(49, 50);
        let subscriber = subscriber(&managers);

        assert!(matches!(
            rule_ops(
                &managers,
                &subscriber,
                &BattleEvent::Kind(EventKind::RoundEnd)
            )
            .unwrap()
            .as_slice(),
            [RuleOp::Command(BattleCommand::Buff(BuffCommand::Remove(
                BuffRemove {
                    selector: BuffRemoveSelector::Uid(2),
                    ..
                }
            )))]
        ));
    }

    #[test]
    fn round_end_commits_the_captured_zero_cost_channel_sequence() {
        use crate::engine::{
            runtime::{
                change::BattleChange,
                determinism::RoundDeterminism,
                drain::{self, ReactionLane},
                record::{FrameItem, FrameOwner, SemanticFrame},
            },
            skill::{
                effect::SkillEffectCatalog,
                target::{TargetContext, TargetPool},
            },
        };

        #[derive(Debug, PartialEq, Eq)]
        enum Step {
            ResourceSnapshot,
            ChildSkill,
            ChannelState,
            Marker,
        }

        fn steps(frame: &SemanticFrame, output: &mut Vec<Step>) {
            if matches!(
                frame.owner,
                FrameOwner::Skill {
                    skill_id: 31280151,
                    ..
                }
            ) {
                output.push(Step::ChildSkill);
            }
            for item in &frame.items {
                match item {
                    FrameItem::Change(change) => match change.as_ref() {
                        BattleChange::Buff(change)
                            if change.change.refreshed.iter().any(|refresh| {
                                refresh.after.buff_id == Some(31280113)
                                    && refresh.before.layer == Some(110)
                                    && refresh.after.layer == Some(110)
                            }) =>
                        {
                            output.push(Step::ResourceSnapshot)
                        }
                        BattleChange::Buff(change)
                            if change.change.refreshed.iter().any(|refresh| {
                                refresh.after.buff_id == Some(31280115)
                                    && refresh.after.act_info.iter().any(|info| {
                                        info.act_id == Some(1031)
                                            && info.str_param.as_deref() == Some("50")
                                    })
                            }) =>
                        {
                            output.push(Step::ChannelState)
                        }
                        BattleChange::BuffActInfoMarker(marker)
                            if marker.act_id == 1031
                                && marker.str_param.as_deref() == Some("50") =>
                        {
                            output.push(Step::Marker)
                        }
                        _ => {}
                    },
                    FrameItem::Child(child) => steps(child, output),
                    FrameItem::Cue(_) => {}
                }
            }
        }

        let fight = fight(110, 0);
        let pool = TargetPool::from_fight(&fight);
        let mut managers = BattleManagers::seeded(&fight);
        let catalog = SkillEffectCatalog::from_roots(config::configs::get(), [], [31280115]);
        let result = drain::run_group_event(
            &mut managers,
            &pool,
            &catalog,
            &mut RoundDeterminism::default(),
            TargetContext {
                current_round: 2,
                ..Default::default()
            },
            BattleEvent::Kind(EventKind::RoundEnd),
            ReactionLane::BuffActs,
            Some(&[10]),
        )
        .unwrap();

        let mut observed = Vec::new();
        for frame in &result.frames {
            steps(frame, &mut observed);
        }
        assert_eq!(
            observed,
            [
                Step::ResourceSnapshot,
                Step::ChildSkill,
                Step::ChannelState,
                Step::Marker,
            ]
        );
        assert_eq!(managers.buff.buff_id_or_type_amount(10, 31280113), 110);
        assert!(managers
            .buff
            .snapshot(10, 2)
            .unwrap()
            .act_info
            .iter()
            .any(|info| info.act_id == Some(1031) && info.str_param.as_deref() == Some("50")));
    }
}
