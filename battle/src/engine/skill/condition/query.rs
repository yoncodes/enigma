use super::{ParsedCondition, ParsedConditionKind};

pub fn is_random(condition: &ParsedCondition) -> bool {
    matches!(
        condition.kind,
        ParsedConditionKind::Random { .. } | ParsedConditionKind::Rouge2AttributeRandom { .. }
    )
}

pub fn find<'a>(
    conditions: &'a [ParsedCondition],
    predicate: &impl Fn(&ParsedCondition) -> bool,
) -> Option<&'a ParsedCondition> {
    conditions
        .iter()
        .find_map(|condition| find_one(condition, predicate))
}

pub fn find_one<'a>(
    condition: &'a ParsedCondition,
    predicate: &impl Fn(&ParsedCondition) -> bool,
) -> Option<&'a ParsedCondition> {
    if predicate(condition) {
        return Some(condition);
    }
    let ParsedConditionKind::Any(groups) = &condition.kind else {
        return None;
    };
    groups.iter().find_map(|group| find(group, predicate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::skill::condition::none::NoneMode;

    #[test]
    fn finds_parsed_semantics_inside_any_group() {
        let nested = ParsedCondition {
            opcode: 0,
            type_name: String::new(),
            kind: ParsedConditionKind::Any(vec![vec![ParsedCondition {
                opcode: 210,
                type_name: String::new(),
                kind: ParsedConditionKind::None(NoneMode::SkillAction),
                raw_args: Vec::new(),
            }]]),
            raw_args: Vec::new(),
        };

        assert!(
            find(&[nested], &|condition| matches!(
                condition.kind,
                ParsedConditionKind::None(NoneMode::SkillAction)
            ))
            .is_some()
        );
    }
}
