use crate::engine::skill::{
    effect::catalog::RuleIssue,
    rule::{DefinitionKey, route::RouteError},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillOpError {
    MissingSkill(i32),
    InvalidSkillDefinition {
        skill_id: i32,
        issues: Vec<RuleIssue>,
    },
    MissingSourceEntity(i64),
    MissingTriggerContext(i32),
    UncompiledRoute {
        skill_id: i32,
        route: RouteError,
    },
    UnregisteredBehavior {
        opcode: i32,
        type_name: String,
    },
    UnregisteredBuffAct {
        opcode: i32,
        type_name: String,
    },
    MissingBehaviorOp {
        skill_id: i32,
        key: DefinitionKey,
    },
    AmbiguousConditionDriver {
        skill_id: i32,
        opcode: i32,
    },
    UnsupportedAssassinationReactionCardinality {
        skill_id: i32,
        buff_id: i32,
        count: usize,
    },
}

impl From<crate::engine::skill::subscriber::SubscriberError> for SkillOpError {
    fn from(error: crate::engine::skill::subscriber::SubscriberError) -> Self {
        match error {
            crate::engine::skill::subscriber::SubscriberError::MissingSkill {
                skill_id, ..
            } => Self::MissingSkill(skill_id),
            crate::engine::skill::subscriber::SubscriberError::UncompiledRoute {
                skill_id,
                route,
            } => Self::UncompiledRoute { skill_id, route },
        }
    }
}
