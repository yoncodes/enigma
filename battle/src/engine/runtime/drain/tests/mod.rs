use sonettobuf::{
    BuffInfo, CardInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute, HeroExAttribute,
    PowerInfo,
};

use super::*;
use crate::engine::{
    entity::attr::AttrId,
    manager::{
        buff::{BuffCommand, BuffGrant, BuffGrantRelation, CommandOrigin, RelatedBuffGrant},
        card::{CardChangeKind, CardCommand, CardReplaceOwnerSkills, CardSetup},
        eureka::{EUREKA_RESOURCE_ID, EurekaChange, EurekaCommand},
        ex_point::{ExPointChange, ExPointCommand},
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
        rule::{DefinitionKey, RuleDomain, output::BattleCommand, route::ConditionRoute},
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
