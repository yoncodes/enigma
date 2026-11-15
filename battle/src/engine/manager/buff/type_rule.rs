#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BuffTypeRule {
    /// Rejects grants of these buff types while held.
    Refuse(&'static [i32]),
}

macro_rules! buff_type_rules {
    ($($(#[doc = $doc:literal])* $type_id:literal => $rule:expr;)*) => {
        pub(super) fn rule(type_id: i32) -> Option<BuffTypeRule> {
            match type_id {
                $($type_id => Some($rule),)*
                _ => None,
            }
        }

        #[cfg(test)]
        const TYPE_IDS: &[i32] = &[$($type_id),*];
    };
}

buff_type_rules! {
    /// J ultimate: "does not gain [Burn] stacks from this attack".
    30940191 => BuffTypeRule::Refuse(&[4150001]);
    /// Refuse Absolute Defense.
    8092 => BuffTypeRule::Refuse(&[1026, 1027]);
    /// Refuse Chorus.
    8304 => BuffTypeRule::Refuse(&[8111]);
}

pub(super) fn refuses(holder_type: i32, incoming_type: i32) -> bool {
    matches!(rule(holder_type), Some(BuffTypeRule::Refuse(types)) if types.contains(&incoming_type))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_names_configured_buff_types() {
        crate::test_support::init_config();
        let types = &crate::test_support::game_data().skill_bufftype;
        for &type_id in TYPE_IDS {
            assert!(types.get(type_id).is_some(), "rule key {type_id}");
            let Some(BuffTypeRule::Refuse(refused)) = rule(type_id) else {
                panic!("rule key {type_id} has no rule");
            };
            for &refused in refused {
                assert!(types.get(refused).is_some(), "{type_id} refuses {refused}");
            }
        }
    }
}
