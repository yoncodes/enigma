use sonettobuf::{
    BuffActInfo, BuffInfo, CardInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute,
    HeroExAttribute, PowerInfo,
};

use super::*;
use crate::engine::{
    entity::attr::AttrId,
    manager::{
        buff::{
            BuffCommand, BuffGrant, BuffGrantRelation, BuffRemove, BuffRemoveSelector,
            CommandOrigin, RelatedBuffGrant, SourceRelativeBuffGrant,
        },
        card::{CARD_PLAY_ORIGIN, CardChangeKind, CardCommand, CardReplaceOwnerSkills, CardSetup},
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand},
        ex_point::{ExPointChange, ExPointCommand, LowestExPointChange},
    },
    mechanic::shell::ShellCommand,
    runtime::change::BattleChange,
    skill::{
        action::{ActionEvent, SkillExecutionMode, SkillInvocation, SkillRequest, SkillTarget},
        behavior::classify::BehaviorSpec,
        condition::{
            ParsedCondition, ParsedConditionKind, lifecycle::LifecycleMode, none::NoneMode,
        },
        effect::{ParsedBehavior, ParsedSkillEffect, SkillEffectSlot},
        rule::{
            ConfiguredRuleKey, DefinitionKey, RuleDomain, output::BattleCommand,
            route::ConditionRoute,
        },
        target::TargetRequest,
    },
};

mod frames;
mod queue;
mod reactions;
mod safety;
mod setup;

#[test]
fn normal_buff_consequence_selects_the_root_uid_lane() {
    let grant = BuffGrant {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "AddBuff"),
        },
        source_uid: 10,
        target_uid: 10,
        buff_id: 31430151,
        amount: Some(1),
        occurrences: 1,
        child_uid_reservations: 0,
    };

    assert_eq!(
        attach_buff_grant_relation(
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant))),
            crate::engine::skill::condition::registry::ConsequencePolicy::NormalBuffGrant,
        ),
        RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantRelated(
            RelatedBuffGrant {
                grant,
                relation: BuffGrantRelation::Normal,
            }
        )))
    );
}

#[test]
fn source_relative_grant_keeps_snapshot_when_selecting_a_uid_lane() {
    let grant = BuffGrant {
        origin: CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(1, "AddBuff"),
        },
        source_uid: 10,
        target_uid: 11,
        buff_id: 31440116,
        amount: None,
        occurrences: 2,
        child_uid_reservations: 0,
    };
    let act_info = vec![BuffActInfo {
        act_id: Some(1131),
        str_param: Some("203#600".to_owned()),
        ..Default::default()
    }];

    use crate::engine::skill::condition::registry::ConsequencePolicy;
    for (consequence, relation) in [
        (ConsequencePolicy::ChildBuffGrant, BuffGrantRelation::Child),
        (
            ConsequencePolicy::NormalBuffGrant,
            BuffGrantRelation::Normal,
        ),
    ] {
        assert_eq!(
            attach_buff_grant_relation(
                RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantSourceRelative(
                    SourceRelativeBuffGrant {
                        grant,
                        act_info: act_info.clone(),
                        relation: None,
                    },
                ))),
                consequence,
            ),
            RuleOp::Command(BattleCommand::Buff(BuffCommand::GrantSourceRelative(
                SourceRelativeBuffGrant {
                    grant,
                    act_info: act_info.clone(),
                    relation: Some(relation),
                },
            )))
        );
    }
}
