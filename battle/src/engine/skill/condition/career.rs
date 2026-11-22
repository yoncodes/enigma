use crate::engine::skill::condition::parse::{ConditionCompare, ParsedConditionKind};

pub fn target_career(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::TargetCareer(parse_args(args)?))
}

pub fn parse_career_check(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::TargetSharesCasterCareer {
        param: args.first().and_then(|arg| arg.parse().ok()).unwrap_or(0),
    })
}

pub fn parse_per_target_career_count(
    _: i32,
    _: &str,
    args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::PerTargetCareerCount {
        careers: parse_list(args.first()?)?,
        threshold: args.last().and_then(|arg| arg.parse().ok()).unwrap_or(0),
    })
}

pub fn per_team_career_count(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    let [career, divisor, scope] = args else {
        return None;
    };
    let career = career.parse().ok()?;
    let divisor = divisor.parse().ok()?;
    let opposing_team = match scope.as_str() {
        "1" => false,
        "2" => true,
        _ => return None,
    };
    if career <= 0 || divisor <= 0 {
        return None;
    }
    Some(ParsedConditionKind::PerTeamCareerCount {
        career,
        divisor,
        opposing_team,
    })
}

pub fn team_career_count_at_least(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::TeamCareerCount {
        careers: parse_list(args.first()?)?,
        compare: ConditionCompare::GreaterThanOrEqual,
        threshold: args.get(1)?.parse().ok()?,
    })
}

pub fn team_career_count_at_most(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::TeamCareerCount {
        careers: parse_list(args.first()?)?,
        compare: ConditionCompare::LessThanOrEqual,
        threshold: args.get(1)?.parse().ok()?,
    })
}

pub fn target_team_career_count(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    if args.len() != 4 {
        return None;
    }
    let career = args[0].parse().ok()?;
    let compare = match args[1].as_str() {
        "1" => ConditionCompare::GreaterThanOrEqual,
        "3" => ConditionCompare::Equal,
        _ => return None,
    };
    let threshold = args[2].parse().ok()?;
    let scope = args[3].parse::<i32>().ok()?;
    if career <= 0 || threshold <= 0 || scope != 1 {
        return None;
    }
    Some(ParsedConditionKind::TeamCareerCount {
        careers: vec![career],
        compare,
        threshold,
    })
}

pub fn natural_ally_count(_: i32, _: &str, args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::PerTargetCareerCount {
        careers: vec![1, 2, 3, 4],
        threshold: args.first()?.parse().ok()?,
    })
}

fn parse_args(args: &[String]) -> Option<Vec<i32>> {
    args.iter().map(|arg| arg.parse().ok()).collect()
}

fn parse_list(raw: &str) -> Option<Vec<i32>> {
    raw.split([',', '，'])
        .map(|part| part.trim().parse().ok())
        .collect()
}

#[cfg(test)]
mod tests;
