use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffSetState},
    },
    skill::{
        action::{SkillInvocation, SkillRequest},
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};
use sonettobuf::effect_type_enum::EffectType;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [threshold, justice, peace, balance]
        if *threshold > 0 && [justice, peace, balance].iter().all(|skill_id| **skill_id > 0))
}

pub fn append(current: Option<&str>, act_id: i32, color: i32, count: i32) -> Option<String> {
    if !(1..=3).contains(&color) || count <= 0 {
        return None;
    }
    let mut colors = parse(current, act_id)?;
    colors.extend(std::iter::repeat_n(color, count as usize));
    Some(format_state(act_id, &colors))
}

pub fn rule_ops(
    managers: &BattleManagers,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    let BattleEvent::AllyAction(_) = event else {
        return None;
    };
    let [threshold, justice_skill, peace_skill, balance_skill] = subscriber.args.as_slice() else {
        return None;
    };
    let buff = managers
        .buff
        .snapshot(subscriber.owner_uid, subscriber.buff_uid)?;
    let colors = parse(
        buff.act_common_params.as_deref(),
        subscriber.key.definition.opcode,
    )?;
    let threshold = usize::try_from(*threshold).ok()?;
    if colors.len() < threshold {
        return Some(Vec::new());
    }

    let (consumed, remaining) = colors.split_at(threshold);
    let (justice_twice, peace_twice) = totals(consumed);
    let skill_id = match justice_twice.cmp(&peace_twice) {
        std::cmp::Ordering::Greater => *justice_skill,
        std::cmp::Ordering::Less => *peace_skill,
        std::cmp::Ordering::Equal => *balance_skill,
    };
    let origin = super::command_origin(subscriber)?;
    let params = if remaining.is_empty() {
        String::new()
    } else {
        format_state(subscriber.key.definition.opcode, remaining)
    };

    Some(vec![
        RuleOp::Command(BattleCommand::Buff(BuffCommand::SetStateSnapshot(
            BuffSetState {
                origin,
                target_uid: subscriber.owner_uid,
                buff_uid: subscriber.buff_uid,
                params: Some(params),
                act_info: None,
                ex_info: None,
            },
        ))),
        RuleOp::EffectMarker {
            target_uid: subscriber.owner_uid,
            effect_type: EffectType::Redorbluecountexskill as i32,
            effect_num: 0,
            config_effect: 0,
            reserve_id: Some(i64::from(skill_id)),
            reserve_str: Some(format!(
                "{}#{}",
                decimal_half(justice_twice),
                decimal_half(peace_twice)
            )),
        },
        RuleOp::Skill(SkillInvocation::from(SkillRequest {
            source_uid: subscriber.owner_uid,
            skill_id,
        })),
    ])
}

fn parse(current: Option<&str>, act_id: i32) -> Option<Vec<i32>> {
    let Some(current) = current.filter(|current| !current.is_empty()) else {
        return Some(Vec::new());
    };
    let mut parts = current.split('#');
    (parts.next()?.parse::<i32>().ok()? == act_id).then_some(())?;
    parts
        .map(|color| {
            color
                .parse::<i32>()
                .ok()
                .filter(|color| (1..=3).contains(color))
        })
        .collect()
}

fn format_state(act_id: i32, colors: &[i32]) -> String {
    std::iter::once(act_id.to_string())
        .chain(colors.iter().map(i32::to_string))
        .collect::<Vec<_>>()
        .join("#")
}

fn totals(colors: &[i32]) -> (i32, i32) {
    colors
        .iter()
        .fold((0, 0), |(justice, peace), color| match color {
            1 => (justice, peace + 2),
            2 => (justice + 2, peace),
            3 => (justice + 1, peace + 1),
            _ => (justice, peace),
        })
}

fn decimal_half(value: i32) -> String {
    format!("{}.{}", value / 2, if value % 2 == 0 { 0 } else { 5 })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{event::kind::EventKind, skill::subscriber};
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    #[test]
    fn appends_exact_color_codes_to_the_buff_owned_state() {
        assert_eq!(append(None, 897, 1, 1).as_deref(), Some("897#1"));
        assert_eq!(
            append(Some("897#1#1"), 897, 3, 1).as_deref(),
            Some("897#1#1#3")
        );
        assert!(append(Some("896#1"), 897, 2, 1).is_none());
    }

    #[test]
    fn captured_sequence_selects_peace_and_preserves_live_totals() {
        let colors = [1, 1, 3, 2, 1, 3];
        assert_eq!(totals(&colors), (4, 8));
        assert_eq!(decimal_half(4), "2.0");
        assert_eq!(decimal_half(8), "4.0");
    }

    #[test]
    fn captured_threshold_clears_state_marks_totals_and_casts_peace_skill() {
        crate::test_support::init_config();
        let wire = super::super::wire::find(897, "RedOrBlueCount")
            .expect("captured counter must own its wire markers");
        assert_eq!(
            wire.markers(super::super::wire::WirePhase::Add),
            &[EffectType::Redorbluecount as i32]
        );
        assert_eq!(
            wire.markers(super::super::wire::WirePhase::Refresh),
            &[EffectType::Redorbluecountchange as i32]
        );
        assert_eq!(
            wire.snapshot_reserve_str(Some("897#1#1#3")),
            Some("1#1#3".to_owned())
        );
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    team_type: Some(1),
                    current_hp: Some(1),
                    buffs: vec![BuffInfo {
                        uid: Some(1195),
                        buff_id: Some(31100551),
                        from_uid: Some(1),
                        act_common_params: Some("897#1#1#3#2#1#3".to_owned()),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let managers = BattleManagers::seeded(&fight);
        let subscriber = subscriber::for_active_buffs(&managers, EventKind::AllyAction)
            .into_iter()
            .find(|subscriber| subscriber.key.definition.opcode == 897)
            .expect("captured counter buff must subscribe to allied actions");

        let ops = rule_ops(
            &managers,
            &subscriber,
            &BattleEvent::AllyAction(Default::default()),
        )
        .expect("captured counter state must resolve");

        assert!(matches!(
            ops.as_slice(),
            [
                RuleOp::Command(BattleCommand::Buff(BuffCommand::SetStateSnapshot(state))),
                RuleOp::EffectMarker {
                    effect_type,
                    reserve_id: Some(31100563),
                    reserve_str: Some(totals),
                    ..
                },
                RuleOp::Skill(invocation),
            ] if state.params.as_deref() == Some("")
                && *effect_type == EffectType::Redorbluecountexskill as i32
                && totals == "2.0#4.0"
                && invocation.plan.skill_id == 31100563
        ));
    }
}
