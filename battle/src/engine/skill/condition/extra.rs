use crate::engine::skill::condition::parse::{ParsedConditionKind, parse_i32_list};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtraSkillKind {
    ExtraAction,
    FollowUp,
    Riposte,
    Reinforced,
    Other(i32),
}

impl ExtraSkillKind {
    pub const fn id(self) -> i32 {
        match self {
            Self::ExtraAction => 1,
            Self::FollowUp => 2,
            Self::Riposte => 3,
            Self::Reinforced => 5,
            Self::Other(value) => value,
        }
    }

    pub const fn is_extra_action(self) -> bool {
        matches!(self, Self::ExtraAction | Self::FollowUp)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExtraActionConditionMode {
    ActiveAction,
    OtherAllyAction,
    DamageAction,
    IncomingAttack,
}

pub fn skill_kind_from_is_extra(is_extra: i32) -> Option<ExtraSkillKind> {
    match is_extra {
        0 => None,
        1 => Some(ExtraSkillKind::ExtraAction),
        2 => Some(ExtraSkillKind::FollowUp),
        3 => Some(ExtraSkillKind::Riposte),
        5 => Some(ExtraSkillKind::Reinforced),
        other => Some(ExtraSkillKind::Other(other)),
    }
}

pub fn active_action(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ExtraAction {
        mode: ExtraActionConditionMode::ActiveAction,
        kinds: parse_i32_list(args.first()?)?,
    })
}

pub fn other_ally_action(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::ExtraAction {
        mode: ExtraActionConditionMode::OtherAllyAction,
        kinds: parse_i32_list(args.first()?)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_extra_keeps_action_followup_and_riposte_distinct() {
        assert_eq!(skill_kind_from_is_extra(0), None);
        assert_eq!(
            skill_kind_from_is_extra(1),
            Some(ExtraSkillKind::ExtraAction)
        );
        assert_eq!(skill_kind_from_is_extra(2), Some(ExtraSkillKind::FollowUp));
        assert_eq!(skill_kind_from_is_extra(3), Some(ExtraSkillKind::Riposte));
        assert!(ExtraSkillKind::ExtraAction.is_extra_action());
        assert!(ExtraSkillKind::FollowUp.is_extra_action());
        assert!(!ExtraSkillKind::Riposte.is_extra_action());
        assert_eq!(
            skill_kind_from_is_extra(5),
            Some(ExtraSkillKind::Reinforced)
        );
    }
}
