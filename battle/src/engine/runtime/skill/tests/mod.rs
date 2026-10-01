use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute, HeroExAttribute};

use super::*;
use crate::engine::{
    entity::attr::AttrId,
    event::{kind::EventKind, payload::BattleEvent},
    manager::card::{CardCommand, CardEnergyChange},
    manager::hp::{DamageEffectKind, HpCommand, HpKill, HpLoss},
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector},
    },
    runtime::determinism::RoundDeterminism,
    skill::{
        action::{
            AdditionalDamageModifier, SkillExecutionMode, SkillInvocation, SkillPhase,
            SkillRequest, SkillTarget,
        },
        behavior::classify::BehaviorSpec,
        condition::{ConditionCompare, ParsedCondition, ParsedConditionKind},
        effect::{
            ParsedBehavior, ParsedSkillEffect, SkillEffectCatalog, SkillEffectSlot,
            catalog::{RuleIssue, RuleIssueReason},
        },
        rule::{
            CommandOrigin, DefinitionKey, RuleDomain, SetupStage,
            output::{BattleCommand, RuleOp},
        },
        target::{TargetContext, TargetPool, TargetRequest},
    },
};

const FIELD_ORIGIN: CommandOrigin = CommandOrigin {
    domain: RuleDomain::Behavior,
    key: DefinitionKey::new(50019, "AddMagicCircle"),
};

fn emit_all_ops(
    mut invocation: SkillInvocation,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    trigger: &SkillOpTrigger,
) -> Result<Vec<RuleOp>, SkillOpError> {
    let mut execution = SkillExecution::new(context);
    let mut ops = Vec::new();
    loop {
        let emission = emit_ops(
            invocation,
            managers,
            pool,
            catalog,
            determinism,
            &mut execution,
            trigger,
        )?;
        ops.extend(emission.ops.into_iter().filter_map(|emission| {
            (!matches!(emission.op, RuleOp::SkillLifecycle(_))).then_some(emission.op)
        }));
        let Some(continuation) = emission.continuation else {
            return Ok(ops);
        };
        invocation = continuation;
    }
}

mod damage;
mod events;
mod lifecycle;
mod planning;
mod routing;
