use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffConsume, BuffGrant, BuffSelector, DepletedBuff},
    },
    skill::{
        behavior::classify::BehaviorKind,
        buff_act::registry::BuffActKind,
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

// Assassination grants Final DMG +5% per 100 excess Critical Technique. A target mark's first
// reaction can convert the attack; its configured rate applies to later reactions in that action.
const TARGET_ASSASSINATION_BASE_RATE: i32 = 50;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetMark {
    pub owner_uid: i64,
    pub buff_id: i32,
    pub rate: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnsupportedReactionCardinality {
    pub skill_id: i32,
    pub buff_id: i32,
    pub count: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AssassinationModifier {
    pub assassinate: bool,
    pub triggered_by_target: bool,
    pub final_damage_bonus: i32,
}

pub fn supports_source_bonus(args: &[i32]) -> bool {
    matches!(args, [rate] if *rate > 0)
}

pub fn supports_target_trigger(args: &[i32]) -> bool {
    matches!(args, [rate, consume, mappings @ ..]
        if *rate > 0
            && *consume > 0
            && !mappings.is_empty()
            && mappings.len() % 2 == 0
            && mappings.iter().all(|skill_id| *skill_id > 0))
}

pub fn parse_target_trigger(raw_args: &[String]) -> Option<Vec<i32>> {
    let [rate, consume, mappings] = raw_args else {
        return None;
    };
    let rate = rate.trim().parse::<i32>().ok()?;
    let consume = consume.trim().parse::<i32>().ok()?;
    let mut values = vec![rate, consume];
    for pair in mappings.split(':') {
        let mut pair = pair.split(',');
        let passive_skill = pair.next()?;
        let active_skill = pair.next()?;
        if pair.next().is_some() {
            return None;
        }
        for skill_id in [passive_skill, active_skill] {
            let skill_id = skill_id.trim().parse::<i32>().ok()?;
            if skill_id <= 0 {
                return None;
            }
            values.push(skill_id);
        }
    }
    supports_target_trigger(&values).then_some(values)
}

// Targets marked when an action begins, with each mark's damage rate. Marks the action inflicts
// itself land after it attacked, and a mark consumed as the attack starts still counts for it.
pub fn marked_targets(managers: &BattleManagers) -> Vec<TargetMark> {
    managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.amount > 0)
        .filter(|feature| super::is_kind(feature, BuffActKind::BeAttackedAssassinate))
        .filter_map(|feature| {
            Some(TargetMark {
                owner_uid: feature.owner_uid,
                buff_id: feature.buff_id,
                rate: *feature.values.get(1)?,
            })
        })
        .collect()
}

fn configured_target_trigger_count(
    catalog: &SkillEffectCatalog,
    skill_id: i32,
    buff_id: i32,
) -> usize {
    catalog
        .get(skill_id)
        .map(|effect| {
            effect
                .slots
                .iter()
                .filter(|slot| {
                    slot.behavior.spec.kind == BehaviorKind::AddBuff
                        && slot.behavior.arg(0) == Some(buff_id)
                })
                .count()
        })
        .unwrap_or_default()
}

fn target_rate_applies(
    skill_id: i32,
    buff_id: i32,
    inherent: bool,
    count: usize,
) -> Result<bool, UnsupportedReactionCardinality> {
    if !inherent && count > 2 {
        return Err(UnsupportedReactionCardinality {
            skill_id,
            buff_id,
            count,
        });
    }
    Ok(inherent || count == 2)
}

pub fn rate_marks_for_skill(
    marks: &[TargetMark],
    catalog: &SkillEffectCatalog,
    skill_id: i32,
) -> Result<Vec<(i64, i32)>, UnsupportedReactionCardinality> {
    let inherent = catalog.is_assassinate(skill_id);
    marks
        .iter()
        .filter_map(|mark| {
            let count = configured_target_trigger_count(catalog, skill_id, mark.buff_id);
            match target_rate_applies(skill_id, mark.buff_id, inherent, count) {
                Ok(true) => Some(Ok((mark.owner_uid, mark.rate))),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect()
}

pub fn target_modifier(
    managers: &BattleManagers,
    source_uid: i64,
    target_uid: i64,
    already_assassinate: bool,
    marks_at_action_start: Option<&[TargetMark]>,
    rate_marks_at_action_start: Option<&[(i64, i32)]>,
) -> AssassinationModifier {
    let features = managers.buff.active_features(&managers.hp);
    let live_marks = marked_targets(managers);
    let marks = match marks_at_action_start {
        Some(marks) => marks,
        None => &live_marks,
    };
    let marked = marks.iter().any(|mark| mark.owner_uid == target_uid);
    let target_rate = match rate_marks_at_action_start {
        Some(marks) => marks
            .iter()
            .filter(|(owner_uid, _)| *owner_uid == target_uid)
            .map(|(_, mark_rate)| *mark_rate)
            .max()
            .unwrap_or_default(),
        None if already_assassinate => live_marks
            .iter()
            .filter(|mark| mark.owner_uid == target_uid)
            .map(|mark| mark.rate)
            .max()
            .unwrap_or_default(),
        None => 0,
    };
    let assassinate = already_assassinate || marked;
    let source_rate = features
        .iter()
        .filter(|feature| feature.owner_alive && feature.owner_uid == source_uid)
        .filter(|feature| super::is_kind(feature, BuffActKind::AddAssassinateY))
        .filter_map(|feature| feature.values.get(1))
        .copied()
        .sum::<i32>();
    let technique_excess = (managers.origin_attribute(source_uid, AttrId::CriticalTechnique)
        - managers.origin_attribute(target_uid, AttrId::CriticalTechnique))
    .max(0);
    AssassinationModifier {
        assassinate,
        triggered_by_target: marked && !already_assassinate,
        final_damage_bonus: i32::from(assassinate)
            * (technique_excess / 100)
            * (TARGET_ASSASSINATION_BASE_RATE + source_rate + target_rate),
    }
}

pub fn rule_ops(
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(subscriber, BuffActKind::BeAttackedAssassinate) {
        return None;
    }
    let Some(action) = event.skill_action_event() else {
        return Some(Vec::new());
    };
    if action.phase != crate::engine::skill::action::SkillPhase::Immediate {
        return Some(Vec::new());
    }
    // "When being attacked, trigger [Assassination] and remove 1 stack", before the attack's own effects.
    if let [_, amount, _, ..] = subscriber.args.as_slice() {
        if !matches!(event, BattleEvent::SkillEffectStarted(_)) {
            return Some(Vec::new());
        }
        if !action.is_attack
            || catalog.damage_rate(action.skill_id) <= 0
            || action.source_uid == subscriber.owner_uid
            || subscriber.amount <= 0
            || !action.target_uids.contains(&subscriber.owner_uid)
            || catalog.is_assassinate(action.skill_id)
        {
            return Some(Vec::new());
        }
        // Separate configured mark applications each trigger this reaction; their layer amounts do not.
        let reaction_count =
            configured_target_trigger_count(catalog, action.skill_id, subscriber.buff_id).max(1);
        return Some(vec![RuleOp::Command(BattleCommand::Buff(
            BuffCommand::Consume(BuffConsume {
                origin: super::command_origin(subscriber)?,
                target_uid: subscriber.owner_uid,
                selector: BuffSelector::Uid(subscriber.buff_uid),
                amount: amount.saturating_mul(i32::try_from(reaction_count).unwrap_or(i32::MAX)),
                depleted: DepletedBuff::RemoveWithZeroLayer,
            }),
        ))]);
    }
    let [active_skill_id] = subscriber.args.as_slice() else {
        return None;
    };
    if !matches!(event, BattleEvent::SkillAction(_))
        || action.source_uid != subscriber.owner_uid
        || action.skill_id != *active_skill_id
    {
        return Some(Vec::new());
    }
    let mut targets = action.target_uids.clone();
    if targets.is_empty() && action.target_uid != 0 {
        targets.push(action.target_uid);
    }
    targets.retain(|target_uid| *target_uid != 0);
    targets.sort_unstable();
    targets.dedup();
    Some(
        targets
            .into_iter()
            .map(|target_uid| {
                RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                    origin: super::command_origin(subscriber).expect("registered buff act"),
                    source_uid: subscriber.owner_uid,
                    target_uid,
                    buff_id: subscriber.buff_id,
                    amount: Some(1),
                    occurrences: 1,
                    child_uid_reservations: 0,
                })))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests;
