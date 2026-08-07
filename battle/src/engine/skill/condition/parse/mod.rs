use config::GameDB;

use crate::engine::skill::condition::{
    buff::BuffConditionMode, extra::ExtraActionConditionMode, lifecycle::LifecycleMode,
    none::NoneMode, registry,
};

#[cfg(test)]
use crate::engine::{
    event::{kind::EventKind, subscription::SubscriptionKey},
    skill::condition::timing::ConditionTiming,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCondition {
    pub opcode: i32,
    pub type_name: String,
    pub kind: ParsedConditionKind,
    pub raw_args: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffAddedScope {
    Owner,
    Team,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedConditionKind {
    Any(Vec<Vec<ParsedCondition>>),
    Not(Box<ParsedConditionKind>),
    Lifecycle(LifecycleMode),
    RoundInterval {
        start_round: i32,
        period: i32,
    },
    ActionOrder(i32),
    ActionOrderRange {
        start: i32,
        count: i32,
    },
    None(NoneMode),
    BuffId {
        mode: BuffConditionMode,
        buff_ids: Vec<i32>,
    },
    BuffIdCount {
        buff_ids: Vec<i32>,
        compare: ConditionCompare,
        threshold: i32,
    },
    BuffIdThreshold {
        buff_ids: Vec<i32>,
        threshold: i32,
    },
    TeamBuffPresence {
        team: i32,
        present: bool,
        buff_id: i32,
    },
    BuffTypeCount {
        type_ids: Vec<i32>,
        compare: ConditionCompare,
        threshold: i32,
    },
    AnyTargetBuffTypeCount {
        type_ids: Vec<i32>,
        threshold: i32,
    },
    BuffGroup(Vec<i32>),
    PerBuffGroupCount {
        group_id: i32,
    },
    NoBuffGroup(Vec<i32>),
    FromBuffAndToBuff {
        from_buff_id: i32,
        to_buff_id: i32,
    },
    SelfBuffTypeTargetBuffTypes {
        self_type_id: i32,
        target_type_ids: Vec<i32>,
    },
    EnemyHighestBuffTypeCount {
        type_id: i32,
        threshold: i32,
    },
    BurnOverflow,
    PerBuffTypeLayer {
        type_ids: Vec<i32>,
        min: i32,
        max: i32,
    },
    BuffStatusCount {
        status_ids: Vec<i32>,
        compare: ConditionCompare,
        threshold: i32,
    },
    PerTeamBuffStatusTypeCount {
        status_ids: Vec<i32>,
        divisor: i32,
        max_count: i32,
    },
    BuffAdded(Vec<i32>),
    BuffRemoved(Vec<i32>),
    AccBuffAddedCount {
        buff_ids: Vec<i32>,
        threshold: i32,
        scope: BuffAddedScope,
    },
    HpPermille {
        compare: ConditionCompare,
        threshold: i32,
    },
    PerHp {
        interval_permille: i32,
    },
    PerLostHp {
        interval_permille: i32,
    },
    TeamLostHpPercent {
        team_type: i32,
        interval_permille: i32,
        max_count: i32,
    },
    BloodPoolMax {
        min: i32,
        max: i32,
    },
    BloodPoolValue {
        min: i32,
        max: i32,
        config_effect: i32,
    },
    CurrentCardEnchant {
        enchant_id: i32,
    },
    HandSkillPresence(Vec<i32>),
    RoundUsedMinimumRank {
        minimum_rank: i32,
        threshold: i32,
    },
    ExPoint {
        compare: ConditionCompare,
        threshold: i32,
    },
    ExPointFull,
    Synchronization {
        threshold: i32,
    },
    PerExPoint {
        threshold: i32,
    },
    ExPointDecrease {
        threshold: i32,
    },
    ExPointLost,
    ExPointIncrChange {
        threshold: i32,
        kind: i32,
        scope: ExPointIncreaseScope,
    },
    Random {
        threshold: i32,
    },
    PowerCompare {
        compare_code: i32,
        power_id: i32,
        threshold: i32,
    },
    PowerRatio {
        power_id: i32,
        compare_code: i32,
        threshold_permille: i32,
    },
    PowerIncrChange {
        power_id: i32,
        compare_code: i32,
        threshold: i32,
    },
    PowerOverflow {
        power_id: i32,
        max_count: i32,
    },
    PowerConsumed {
        power_id: i32,
        max_count: i32,
    },
    PerConduitCurrentCost {
        threshold: i32,
    },
    ConduitExPoint {
        compare_code: i32,
        threshold: i32,
    },
    ConduitSkillGroup {
        group: i32,
    },
    CurrentEntityPowerDecrease,
    PowerUseAddBuff {
        threshold: i32,
    },
    LostPower {
        power_id: i32,
        threshold: i32,
    },
    TargetAttacked,
    AllyAttacked,
    ShareDamage,
    Assassinate,
    TeammateInjuryCount {
        persistent: bool,
        threshold: i32,
    },
    TeamInjuryCountRound {
        max_count: i32,
    },
    EntityDead,
    TeammateDead,
    EnemyDead,
    TargetGuardBroken,
    SingleKillCount {
        threshold: i32,
    },
    PerKillCount {
        divisor: i32,
    },
    TeamEntityExited {
        max_count: i32,
    },
    MultiHpSegment(i32),
    TargetCareer(Vec<i32>),
    TargetSharesCasterCareer {
        param: i32,
    },
    PerTargetCareerCount {
        careers: Vec<i32>,
        threshold: i32,
    },
    TeamCareerCount {
        careers: Vec<i32>,
        compare: ConditionCompare,
        threshold: i32,
    },
    OtherAllyDamageTypeCount {
        damage_type: crate::engine::skill::target::EntityDamageType,
        max_count: i32,
    },
    ActiveSkillId(Vec<i32>),
    CanUseSkill(Vec<i32>),
    ActiveUseSkill {
        slot: i32,
    },
    UseSkillRank(Vec<i32>),
    UseHurtSkill,
    SpecificSkill {
        group: i32,
        rank: i32,
    },
    ReceivedSpecificSkill {
        group: i32,
        rank: i32,
    },
    UseExSkill,
    TargetUseExSkill,
    TeammateUseExSkill,
    ActiveSkillRank {
        compare: ConditionCompare,
        ranks: Vec<i32>,
    },
    ActiveSkillType(i32),
    ActiveSkillEffectTag(Vec<i32>),
    DamageTargetCountKind(i32),
    SourceDamageType(crate::engine::skill::target::EntityDamageType),
    AttackerDamageType(crate::engine::skill::target::EntityDamageType),
    AttackCrit,
    BeforeCrit,
    GuardBroken,
    EntityBroken,
    HurtRestrained,
    HurtNotRestrained,
    EntityCount {
        scope: EntityCountScope,
        compare: ConditionCompare,
        count: i32,
    },
    SummonedCount {
        summoned_id: i32,
        required_level: i32,
        compare: ConditionCompare,
        count: i32,
    },
    GroupSummonedCount {
        owner_model_id: i32,
        required_level: i32,
        compare: ConditionCompare,
        count: i32,
    },
    BattleTagCount {
        tag_id: i32,
        compare: ConditionCompare,
        threshold: i32,
    },
    TargetIdentity {
        mode: TargetIdentityMode,
        value: i32,
    },
    TeamContainsModels(Vec<i32>),
    TeamModelPresence {
        model_ids: Vec<i32>,
        present: bool,
    },
    ExtraAction {
        mode: ExtraActionConditionMode,
        kinds: Vec<i32>,
    },
    InMagicCircleId(Vec<i32>),
    NotInMagicCircleId(Vec<i32>),
    AddedMagicCircle(Vec<i32>),
    RemovedMagicCircle(Vec<i32>),
    BuffFeatureTriggered {
        act_id: i32,
    },
    MasterHalo,
    NoActionRound,
    Unsupported(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExPointIncreaseScope {
    SelfOnly,
    OtherAlly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionCompare {
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetIdentityMode {
    TargetIsSelf,
    TargetIsAllyNotSelf,
    TargetModelId,
    TargetPosition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityCountScope {
    EnemyTargets,
    AliveEnemies,
    AliveEnemiesIncludeSp,
    AliveTeammates,
    AliveOtherTeammates,
    AliveTeammatesNoSp,
    TeamSize,
    HeroCount,
}

impl ParsedCondition {
    pub fn always() -> Self {
        Self {
            opcode: 0,
            type_name: "None".into(),
            kind: ParsedConditionKind::None(NoneMode::Always),
            raw_args: Vec::new(),
        }
    }

    pub fn allows_active_skill(&self) -> bool {
        matches!(
            self.kind,
            ParsedConditionKind::None(
                NoneMode::Always
                    | NoneMode::Unconditional
                    | NoneMode::SkillAction
                    | NoneMode::SkillActionStart
                    | NoneMode::SkillActionAfterDamage
                    | NoneMode::SkillActionAfterHit
                    | NoneMode::SkillAfterAttack
                    | NoneMode::SkillCast
            )
        )
    }
}

pub fn parse_conditions(db: &GameDB, raw: &str) -> Vec<ParsedCondition> {
    if raw.trim().is_empty() {
        return vec![ParsedCondition::always()];
    }

    let groups = raw
        .split('|')
        .map(|group| {
            group
                .split('&')
                .map(|part| parse_condition(db, part))
                .collect::<Option<Vec<_>>>()
                .filter(|conditions| !conditions.is_empty())
        })
        .collect::<Option<Vec<_>>>();

    let Some(groups) = groups else {
        return vec![unsupported_condition("Malformed")];
    };

    if groups.is_empty() {
        vec![ParsedCondition::always()]
    } else if groups.len() == 1 {
        groups.into_iter().next().unwrap()
    } else {
        vec![ParsedCondition {
            opcode: 0,
            type_name: "Any".into(),
            kind: ParsedConditionKind::Any(groups),
            raw_args: Vec::new(),
        }]
    }
}

fn parse_condition(db: &GameDB, raw: &str) -> Option<ParsedCondition> {
    let negated = raw.trim_end().ends_with(['!', '！']);
    let clean = raw.trim().trim_end_matches(['!', '！']);
    let mut parts = clean
        .split('#')
        .map(str::trim)
        .filter(|part| !part.is_empty());
    let opcode = parts.next()?.parse().ok()?;
    let raw_args = parts.map(str::to_owned).collect::<Vec<_>>();
    let row = db.skill_behavior_condition.get(opcode)?;
    let mut kind = registry::parse(opcode, &row.r#type, &raw_args)
        .unwrap_or_else(|| ParsedConditionKind::Unsupported(row.r#type.clone()));
    if negated {
        kind = negate_kind(kind);
    }

    Some(ParsedCondition {
        opcode,
        type_name: row.r#type.clone(),
        kind,
        raw_args,
    })
}

pub(super) fn multi_hp_segment(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::MultiHpSegment(first_i32(raw_args)?))
}

pub(super) fn hp_less(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::HpPermille {
        compare: ConditionCompare::LessThan,
        threshold: first_i32(raw_args)?,
    })
}

pub(super) fn hp_more(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::HpPermille {
        compare: ConditionCompare::GreaterThan,
        threshold: raw_args.first().and_then(|arg| parse_i32(arg)).unwrap_or(0),
    })
}

pub(super) fn random(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::Random {
        threshold: first_i32(raw_args)?.clamp(0, 1000),
    })
}

pub(super) fn damage_target_count_kind(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    Some(ParsedConditionKind::DamageTargetCountKind(first_i32(
        raw_args,
    )?))
}

pub(super) fn hero_reality(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    raw_args
        .is_empty()
        .then_some(ParsedConditionKind::SourceDamageType(
            crate::engine::skill::target::EntityDamageType::Reality,
        ))
}

pub(super) fn hero_mental(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    raw_args
        .is_empty()
        .then_some(ParsedConditionKind::SourceDamageType(
            crate::engine::skill::target::EntityDamageType::Mental,
        ))
}

pub(super) fn reality_damage(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    raw_args
        .is_empty()
        .then_some(ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Reality,
        ))
}

pub(super) fn mental_damage(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    raw_args
        .is_empty()
        .then_some(ParsedConditionKind::AttackerDamageType(
            crate::engine::skill::target::EntityDamageType::Mental,
        ))
}

pub(super) fn attack_crit(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    raw_args
        .is_empty()
        .then_some(ParsedConditionKind::AttackCrit)
}

pub(super) fn before_crit(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    raw_args
        .is_empty()
        .then_some(ParsedConditionKind::BeforeCrit)
}

pub(super) fn hurt_restrained(_: i32, _: &str, raw_args: &[String]) -> Option<ParsedConditionKind> {
    raw_args
        .is_empty()
        .then_some(ParsedConditionKind::HurtRestrained)
}

pub(super) fn hurt_not_restrained(
    _: i32,
    _: &str,
    raw_args: &[String],
) -> Option<ParsedConditionKind> {
    raw_args
        .is_empty()
        .then_some(ParsedConditionKind::HurtNotRestrained)
}

fn negate_kind(kind: ParsedConditionKind) -> ParsedConditionKind {
    match kind {
        ParsedConditionKind::BuffId { mode, buff_ids } => ParsedConditionKind::BuffId {
            mode: match mode {
                BuffConditionMode::Present | BuffConditionMode::PresentAndConsume => {
                    BuffConditionMode::Absent
                }
                BuffConditionMode::Absent => BuffConditionMode::Present,
                BuffConditionMode::ExactPresent => BuffConditionMode::ExactAbsent,
                BuffConditionMode::ExactAbsent => BuffConditionMode::ExactPresent,
            },
            buff_ids,
        },
        ParsedConditionKind::HpPermille { compare, threshold } => ParsedConditionKind::HpPermille {
            compare: negate_compare(compare),
            threshold,
        },
        ParsedConditionKind::ExPoint { compare, threshold } => ParsedConditionKind::ExPoint {
            compare: negate_compare(compare),
            threshold,
        },
        ParsedConditionKind::ActiveSkillRank { compare, ranks } => {
            ParsedConditionKind::ActiveSkillRank {
                compare: negate_compare(compare),
                ranks,
            }
        }
        ParsedConditionKind::EntityCount {
            scope,
            compare,
            count,
        } => ParsedConditionKind::EntityCount {
            scope,
            compare: negate_compare(compare),
            count,
        },
        ParsedConditionKind::SummonedCount {
            summoned_id,
            required_level,
            compare,
            count,
        } => ParsedConditionKind::SummonedCount {
            summoned_id,
            required_level,
            compare: negate_compare(compare),
            count,
        },
        ParsedConditionKind::BuffIdCount {
            buff_ids,
            compare,
            threshold,
        } => ParsedConditionKind::BuffIdCount {
            buff_ids,
            compare: negate_compare(compare),
            threshold,
        },
        ParsedConditionKind::BuffTypeCount {
            type_ids,
            compare,
            threshold,
        } => ParsedConditionKind::BuffTypeCount {
            type_ids,
            compare: negate_compare(compare),
            threshold,
        },
        ParsedConditionKind::BuffStatusCount {
            status_ids,
            compare,
            threshold,
        } => ParsedConditionKind::BuffStatusCount {
            status_ids,
            compare: negate_compare(compare),
            threshold,
        },
        ParsedConditionKind::TeamCareerCount {
            careers,
            compare,
            threshold,
        } => ParsedConditionKind::TeamCareerCount {
            careers,
            compare: negate_compare(compare),
            threshold,
        },
        ParsedConditionKind::GroupSummonedCount {
            owner_model_id,
            required_level,
            compare,
            count,
        } => ParsedConditionKind::GroupSummonedCount {
            owner_model_id,
            required_level,
            compare: negate_compare(compare),
            count,
        },
        ParsedConditionKind::BattleTagCount {
            tag_id,
            compare,
            threshold,
        } => ParsedConditionKind::BattleTagCount {
            tag_id,
            compare: negate_compare(compare),
            threshold,
        },
        ParsedConditionKind::Unsupported(reason) => {
            ParsedConditionKind::Unsupported(format!("Not({reason})"))
        }
        other => ParsedConditionKind::Not(Box::new(other)),
    }
}

fn negate_compare(compare: ConditionCompare) -> ConditionCompare {
    match compare {
        ConditionCompare::Equal => ConditionCompare::NotEqual,
        ConditionCompare::NotEqual => ConditionCompare::Equal,
        ConditionCompare::GreaterThan => ConditionCompare::LessThanOrEqual,
        ConditionCompare::GreaterThanOrEqual => ConditionCompare::LessThan,
        ConditionCompare::LessThan => ConditionCompare::GreaterThanOrEqual,
        ConditionCompare::LessThanOrEqual => ConditionCompare::GreaterThan,
    }
}

fn unsupported_condition(type_name: impl Into<String>) -> ParsedCondition {
    let type_name = type_name.into();
    ParsedCondition {
        opcode: 0,
        type_name: type_name.clone(),
        kind: ParsedConditionKind::Unsupported(type_name),
        raw_args: Vec::new(),
    }
}

pub(super) fn parse_fixed<const N: usize>(raw_args: &[String]) -> Option<[i32; N]> {
    let values = raw_args
        .iter()
        .map(|arg| parse_i32(arg))
        .collect::<Option<Vec<_>>>()?;
    values.try_into().ok()
}

pub(super) fn first_i32(raw_args: &[String]) -> Option<i32> {
    raw_args.first().and_then(|arg| parse_i32(arg))
}

pub(super) fn parse_i32(raw: &str) -> Option<i32> {
    raw.trim().trim_end_matches(['!', '！']).parse().ok()
}

pub(super) fn parse_i32_list(raw: &str) -> Option<Vec<i32>> {
    raw.trim()
        .trim_end_matches(['!', '！'])
        .split([',', '，'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::parse)
        .collect::<Result<Vec<_>, _>>()
        .ok()
        .filter(|values| !values.is_empty())
}

pub(super) fn parse_i32_args(raw_args: &[String]) -> Option<Vec<i32>> {
    raw_args
        .iter()
        .flat_map(|arg| arg.split([',', '，']))
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| part.trim_end_matches(['!', '！']).parse())
        .collect::<Result<Vec<_>, _>>()
        .ok()
        .filter(|values| !values.is_empty())
}

#[cfg(test)]
mod test;
