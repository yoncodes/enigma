use super::parse::{ParsedConditionKind, TargetIdentityMode, first_i32, parse_i32_list};

pub fn ex_skill_level(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [level] = args else {
        return None;
    };
    let level = level.parse().ok()?;
    (0..=5)
        .contains(&level)
        .then_some(ParsedConditionKind::ExSkillLevel(level))
}

pub fn target_is_self(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    identity(TargetIdentityMode::TargetIsSelf, 0)
}

pub fn target_is_ally_not_self(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
    identity(TargetIdentityMode::TargetIsAllyNotSelf, 0)
}

pub fn target_model(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    identity(TargetIdentityMode::TargetModelId, first_i32(args)?)
}

pub fn team_contains_model(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::TeamContainsModels(parse_i32_list(
        args.first()?,
    )?))
}

pub fn team_model_presence(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [entity_type, model_ids, scope, mode] = args else {
        return None;
    };
    // This exact opcode is a hero-roster gate. Other HasConditionTarget
    // opcodes carry different entity/scope semantics and need their own routes.
    if entity_type != "6" || scope != "0" {
        return None;
    }
    let present = match mode.as_str() {
        "1" => false,
        "2" => true,
        _ => return None,
    };
    let model_ids = model_ids
        .split(',')
        .map(|value| value.trim().parse().ok())
        .collect::<Option<Vec<_>>>()?;
    (!model_ids.is_empty()).then_some(ParsedConditionKind::TeamModelPresence { model_ids, present })
}

fn identity(mode: TargetIdentityMode, value: i32) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::TargetIdentity { mode, value })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_include_hero_595002_is_an_enter_fight_gate() {
        assert!(matches!(
            target_model(595002, "TargetIncludeHero", &["3091".into()]),
            Some(ParsedConditionKind::TargetIdentity {
                mode: TargetIdentityMode::TargetModelId,
                value: 3091,
            })
        ));
    }

    #[test]
    fn ezio_roster_gate_keeps_its_exact_absence_semantics() {
        assert_eq!(
            team_model_presence(
                643004,
                "HasConditionTarget",
                &["6".into(), "3122,3124".into(), "0".into(), "1".into()]
            ),
            Some(ParsedConditionKind::TeamModelPresence {
                model_ids: vec![3122, 3124],
                present: false,
            })
        );
        assert!(
            team_model_presence(
                643004,
                "HasConditionTarget",
                &["3".into(), "5".into(), "0".into(), "1".into()]
            )
            .is_none()
        );
    }

    #[test]
    fn team_contains_hero_preserves_every_configured_model() {
        assert_eq!(
            team_contains_model(1000212, "TeamContainHero", &["3122,3123".into()]),
            Some(ParsedConditionKind::TeamContainsModels(vec![3122, 3123]))
        );
    }
}
