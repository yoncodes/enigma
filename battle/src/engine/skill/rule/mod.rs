pub mod catalog;
pub mod output;
pub mod ownership;
pub mod route;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefinitionKey {
    pub opcode: i32,
    pub type_name: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuleDomain {
    Skill,
    Behavior,
    Condition,
    BuffAct,
    EffectTime,
    Lifecycle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SetupStage {
    BattleStart,
    EnterFight,
    RoundStartCondition,
    RoundStart,
    RoundStartLate,
    RoundTransitionStart,
    EnterBattleStatic,
    Unconditional,
    BuffGate,
    BuffSync,
    CardSetup,
    GeneratedCard,
    AfterRoundStart,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RuleDescriptor {
    pub domain: RuleDomain,
    pub key: DefinitionKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandOrigin {
    pub domain: RuleDomain,
    pub key: DefinitionKey,
}

impl From<RuleDescriptor> for CommandOrigin {
    fn from(value: RuleDescriptor) -> Self {
        Self {
            domain: value.domain,
            key: value.key,
        }
    }
}

impl RuleDescriptor {
    pub const fn new(domain: RuleDomain, key: DefinitionKey) -> Self {
        Self { domain, key }
    }
}

impl DefinitionKey {
    pub const fn new(opcode: i32, type_name: &'static str) -> Self {
        Self { opcode, type_name }
    }

    pub fn matches(&self, opcode: i32, type_name: &str) -> bool {
        self.opcode == opcode && self.type_name == type_name
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleOutcome {
    Handled { emitted: usize },
    ConditionFalse,
    NoTargets,
    NotApplicable(&'static str),
    Malformed(&'static str),
    Unsupported(&'static str),
}

impl HandleOutcome {
    pub fn is_handled(self) -> bool {
        matches!(self, Self::Handled { .. } | Self::NotApplicable(_))
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuleReferences {
    pub skills: Vec<i32>,
    pub buffs: Vec<i32>,
    pub models: Vec<i32>,
    pub summons: Vec<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definition_key_requires_opcode_and_type() {
        let key = DefinitionKey::new(20002, "AddExPoint");

        assert!(key.matches(20002, "AddExPoint"));
        assert!(!key.matches(30001, "AddExPoint"));
        assert!(!key.matches(20002, "DelExPoint"));
    }
}
