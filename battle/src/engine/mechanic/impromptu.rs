use sonettobuf::CardInfo;

use crate::engine::{
    manager::{
        BattleManagers,
        buff::ActiveBuffFeature,
        card::{CardCommand, CardEnergyAllocation, CardQueueUse},
        gauge::{GaugeCommand, GaugeKey, GaugeKind, GaugeManager, GaugeOperation, GaugeOwner},
    },
    skill::{
        buff_act::{self, registry::BuffActKind},
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImpromptuDefinition {
    skill_id: i32,
    damage_up_act_id: i32,
    damage_rate_per_inspiration: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpromptuError {
    MissingTeamEnergy(i32),
    InvalidEmitterTag(i64),
}

impl ImpromptuDefinition {
    pub(crate) const fn new(
        skill_id: i32,
        damage_up_act_id: i32,
        damage_rate_per_inspiration: i32,
    ) -> Self {
        Self {
            skill_id,
            damage_up_act_id,
            damage_rate_per_inspiration,
        }
    }

    pub const fn skill_id(self) -> i32 {
        self.skill_id
    }

    pub const fn damage_up_act_id(self) -> i32 {
        self.damage_up_act_id
    }

    pub const fn damage_rate(self, inspiration: i32) -> i32 {
        inspiration.saturating_mul(self.damage_rate_per_inspiration)
    }

    pub fn card(self, emitter_uid: i64) -> CardInfo {
        CardInfo {
            uid: Some(emitter_uid),
            skill_id: Some(self.skill_id),
            temp_card: Some(false),
            card_type: Some(0),
            hero_id: Some(0),
            status: Some(0),
            target_uid: Some(0),
            energy: Some(0),
            area_red_or_blue: Some(0),
            heat_id: Some(0),
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImpromptuPlan {
    pub source_uid: i64,
    pub skill_id: i32,
    pub inspiration: i32,
    pub attack_count: i32,
    pub damage_rate_opcode: i32,
    pub damage_rate: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ImpromptuEnable {
    pub team: i32,
    pub emitter_uid: i64,
    pub emitter: RuleOp,
    pub team_energy: RuleOp,
    pub inspiration: RuleOp,
}

pub const fn team_energy_key(team: i32) -> GaugeKey {
    GaugeKey {
        kind: GaugeKind::TeamEnergy,
        owner: GaugeOwner::Team(team),
    }
}

pub const fn inspiration_key(emitter_uid: i64) -> GaugeKey {
    GaugeKey {
        kind: GaugeKind::ImpromptuInspiration,
        owner: GaugeOwner::Entity(emitter_uid),
    }
}

pub fn enable_rule_ops(
    definition: Option<ImpromptuDefinition>,
    gauges: &GaugeManager,
    features: &[ActiveBuffFeature],
    emitter_uid: i64,
) -> Vec<ImpromptuEnable> {
    if emitter_uid == 0 {
        return Vec::new();
    }
    emitter_tags(features)
        .into_iter()
        .filter(|feature| {
            gauges.get(team_energy_key(feature.team_type)).is_none()
                || gauges.get(inspiration_key(emitter_uid)).is_none()
        })
        .filter_map(|feature| {
            let origin = buff_act::feature_command_origin(feature)?;
            let definition = definition?;
            Some(ImpromptuEnable {
                team: feature.team_type,
                emitter_uid,
                emitter: RuleOp::Command(BattleCommand::Emitter(
                    crate::engine::manager::emitter::EmitterCommand {
                        origin,
                        operation: crate::engine::manager::emitter::EmitterOperation::Enable(
                            definition,
                        ),
                    },
                )),
                team_energy: enable(origin, team_energy_key(feature.team_type)),
                inspiration: enable(origin, inspiration_key(emitter_uid)),
            })
        })
        .collect()
}

pub fn team_energy_gain_rule_op(
    gauges: &GaugeManager,
    feature: &ActiveBuffFeature,
    delta: i32,
) -> Option<RuleOp> {
    if !buff_act::is_kind(feature, BuffActKind::UseSkillTeamAddEmitterEnergy) || delta <= 0 {
        return None;
    }
    change(
        gauges,
        buff_act::feature_command_origin(feature)?,
        team_energy_key(feature.team_type),
        delta,
    )
}

pub fn eureka_spent_rule_op(
    managers: &BattleManagers,
    owner_uid: i64,
    amount: i32,
) -> Option<RuleOp> {
    if amount <= 0 {
        return None;
    }
    let team = managers.entity.team_type(owner_uid)?;
    let features = managers.buff.active_features(&managers.hp);
    let tag = emitter_tags(&features)
        .into_iter()
        .find(|feature| feature.team_type == team)?;
    change(
        &managers.gauge,
        buff_act::feature_command_origin(tag)?,
        team_energy_key(team),
        amount,
    )
}

pub fn spend_team_energy_rule_op(
    gauges: &GaugeManager,
    emitter_tag: &ActiveBuffFeature,
    amount: i32,
) -> Option<RuleOp> {
    if !buff_act::is_kind(emitter_tag, BuffActKind::EmitterTag) || amount <= 0 {
        return None;
    }
    change(
        gauges,
        buff_act::feature_command_origin(emitter_tag)?,
        team_energy_key(emitter_tag.team_type),
        -amount,
    )
}

pub fn allocate_team_energy_rule_ops(
    managers: &BattleManagers,
    catalog: &SkillEffectCatalog,
    determinism: &mut crate::engine::runtime::determinism::RoundDeterminism,
    team: i32,
) -> Result<Option<Vec<RuleOp>>, ImpromptuError> {
    let features = managers.buff.active_features(&managers.hp);
    let Some(tag) = emitter_tags(&features)
        .into_iter()
        .find(|feature| feature.team_type == team)
    else {
        return Ok(None);
    };
    let available = managers
        .gauge
        .get(team_energy_key(team))
        .ok_or(ImpromptuError::MissingTeamEnergy(team))?
        .current;
    let Some(cards) = determinism
        .take_card_energy_snapshot(managers.card.hand(), available)
        .or_else(|| {
            crate::engine::manager::card::energy::allocate(
                &managers.buff,
                &managers.hp,
                catalog,
                managers.card.hand(),
                available,
                available > 4,
                determinism,
            )
        })
    else {
        return Ok(Some(Vec::new()));
    };
    let allocated = cards
        .iter()
        .zip(managers.card.hand())
        .map(|(after, before)| after.energy.unwrap_or_default() - before.energy.unwrap_or_default())
        .sum::<i32>();
    let origin = buff_act::feature_command_origin(tag)
        .ok_or(ImpromptuError::InvalidEmitterTag(tag.buff_uid))?;
    Ok(Some(vec![
        spend_team_energy_rule_op(&managers.gauge, tag, allocated)
            .ok_or(ImpromptuError::InvalidEmitterTag(tag.buff_uid))?,
        RuleOp::Command(BattleCommand::Card(CardCommand::AllocateEnergy(
            CardEnergyAllocation {
                origin,
                energies: cards
                    .into_iter()
                    .map(|card| card.energy.unwrap_or_default())
                    .collect(),
            },
        ))),
    ]))
}

fn emitter_tags(features: &[ActiveBuffFeature]) -> Vec<&ActiveBuffFeature> {
    let mut tags = features
        .iter()
        .filter(|feature| {
            feature.owner_alive
                && feature.team_type != 0
                && buff_act::is_kind(feature, BuffActKind::EmitterTag)
        })
        .collect::<Vec<_>>();
    tags.sort_by_key(|feature| (feature.team_type, feature.owner_uid, feature.buff_uid));
    tags.dedup_by_key(|feature| feature.team_type);
    tags
}

pub fn collect_inspiration_rule_op(
    gauges: &GaugeManager,
    emitter_tag: &ActiveBuffFeature,
    emitter_uid: i64,
    cards: &[CardInfo],
) -> Option<RuleOp> {
    if !buff_act::is_kind(emitter_tag, BuffActKind::EmitterTag) {
        return None;
    }
    let delta = cards
        .iter()
        .map(|card| card.energy.unwrap_or_default())
        .fold(0_i32, i32::saturating_add);
    if delta <= 0 {
        return None;
    }
    change(
        gauges,
        buff_act::feature_command_origin(emitter_tag)?,
        inspiration_key(emitter_uid),
        delta,
    )
}

pub fn action_queue_committed_rule_ops(
    managers: &BattleManagers,
    team: i32,
    emitter_uid: i64,
    cards: &[CardInfo],
) -> Vec<RuleOp> {
    let features = managers.buff.active_features(&managers.hp);
    let Some(tag) = emitter_tags(&features)
        .into_iter()
        .find(|feature| feature.team_type == team)
    else {
        return Vec::new();
    };
    let current = managers
        .gauge
        .get(inspiration_key(emitter_uid))
        .map(|state| state.current)
        .unwrap_or_default();
    let gained = cards
        .iter()
        .map(|card| card.energy.unwrap_or_default())
        .fold(0_i32, i32::saturating_add);
    let mut ops = collect_inspiration_rule_op(&managers.gauge, tag, emitter_uid, cards)
        .into_iter()
        .collect::<Vec<_>>();
    if current.saturating_add(gained) > 0
        && let Some(definition) = managers.catalog().impromptu_definition()
        && let Some(origin) = buff_act::feature_command_origin(tag)
    {
        ops.push(RuleOp::Command(BattleCommand::Card(
            CardCommand::QueueUseCard(CardQueueUse {
                origin,
                card_index: cards.len() as i32 + 1,
                card: definition.card(emitter_uid),
                team_type: team,
                source_skill_id: 0,
                action: None,
            }),
        )));
    }
    ops
}

pub fn finalize_action_queue_rule_ops(
    managers: &BattleManagers,
    team: i32,
    emitter_uid: i64,
) -> Vec<RuleOp> {
    let features = managers.buff.active_features(&managers.hp);
    let Some(tag) = emitter_tags(&features)
        .into_iter()
        .find(|feature| feature.team_type == team)
    else {
        return Vec::new();
    };
    let Some(origin) = buff_act::feature_command_origin(tag) else {
        return Vec::new();
    };
    let key = inspiration_key(emitter_uid);
    if managers.gauge.get(key).is_none() {
        return Vec::new();
    }
    let ops = vec![
        RuleOp::Command(BattleCommand::Card(CardCommand::ClearEnergy { origin })),
        RuleOp::Command(BattleCommand::Gauge(GaugeCommand::new(
            origin,
            key,
            GaugeOperation::Snapshot,
        ))),
    ];
    ops
}

pub fn build_plan(managers: &BattleManagers, team: i32, emitter_uid: i64) -> Option<ImpromptuPlan> {
    let features = managers.buff.active_features(&managers.hp);
    emitter_tags(&features)
        .into_iter()
        .any(|feature| feature.team_type == team)
        .then_some(())?;
    let inspiration = managers.gauge.get(inspiration_key(emitter_uid))?.current;
    if inspiration <= 0 {
        return None;
    }
    let definition = managers.catalog().impromptu_definition()?;
    Some(ImpromptuPlan {
        source_uid: emitter_uid,
        skill_id: definition.skill_id(),
        inspiration,
        attack_count: crate::engine::skill::buff_act::emitter_num_change::attack_count_for(
            &managers.buff,
            &managers.hp,
            emitter_uid,
        ),
        damage_rate_opcode: definition.damage_up_act_id(),
        damage_rate: definition.damage_rate(inspiration),
    })
}

pub fn resolved_rule_ops(managers: &BattleManagers, team: i32, emitter_uid: i64) -> Vec<RuleOp> {
    let features = managers.buff.active_features(&managers.hp);
    let Some(tag) = emitter_tags(&features)
        .into_iter()
        .find(|feature| feature.team_type == team)
    else {
        return Vec::new();
    };
    reset_inspiration_rule_op(&managers.gauge, tag, emitter_uid)
        .into_iter()
        .collect()
}

pub fn reset_inspiration_rule_op(
    gauges: &GaugeManager,
    emitter_tag: &ActiveBuffFeature,
    emitter_uid: i64,
) -> Option<RuleOp> {
    if !buff_act::is_kind(emitter_tag, BuffActKind::EmitterTag) {
        return None;
    }
    let current = gauges.get(inspiration_key(emitter_uid))?.current;
    change(
        gauges,
        buff_act::feature_command_origin(emitter_tag)?,
        inspiration_key(emitter_uid),
        -current,
    )
}

fn enable(origin: crate::engine::skill::rule::CommandOrigin, key: GaugeKey) -> RuleOp {
    RuleOp::Command(BattleCommand::Gauge(GaugeCommand::new(
        origin,
        key,
        GaugeOperation::Enable { max: None },
    )))
}

fn change(
    gauges: &GaugeManager,
    origin: crate::engine::skill::rule::CommandOrigin,
    key: GaugeKey,
    delta: i32,
) -> Option<RuleOp> {
    (delta != 0 && gauges.get(key).is_some()).then_some(RuleOp::Command(BattleCommand::Gauge(
        GaugeCommand::new(origin, key, GaugeOperation::ChangeValue { delta }),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::manager::BattleManagers;

    #[test]
    fn active_impromptu_without_team_energy_is_an_error() {
        crate::test_support::init_config();
        let fight = sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    team_type: Some(1),
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(20),
                        buff_id: Some(2_240_000),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };

        assert_eq!(
            allocate_team_energy_rule_ops(
                &BattleManagers::seeded(&fight),
                &SkillEffectCatalog::default(),
                &mut crate::engine::runtime::determinism::RoundDeterminism::default(),
                1,
            ),
            Err(ImpromptuError::MissingTeamEnergy(1))
        );
    }

    #[test]
    fn team_energy_and_inspiration_have_distinct_lifetimes() {
        crate::test_support::init_config();
        let tag = feature(10, 1, 20, "EmitterTag", vec![875]);
        let gain = feature(10, 1, 21, "UseSkillTeamAddEmitterEnergy", vec![881, 1, 2]);
        let mut managers = BattleManagers::default();
        let enable = enable_rule_ops(
            crate::catalog::impromptu_definition(crate::test_support::game_data()),
            &managers.gauge,
            &[tag.clone(), gain.clone()],
            99998,
        )
        .pop()
        .unwrap();
        for output in [enable.team_energy, enable.inspiration] {
            let RuleOp::Command(BattleCommand::Gauge(command)) = output else {
                panic!("gauge command");
            };
            managers.execute_gauge(command).unwrap();
        }

        let RuleOp::Command(BattleCommand::Gauge(command)) =
            team_energy_gain_rule_op(&managers.gauge, &gain, 2).unwrap()
        else {
            panic!("gauge command");
        };
        managers.execute_gauge(command).unwrap();
        let cards = [CardInfo {
            energy: Some(3),
            ..Default::default()
        }];
        let RuleOp::Command(BattleCommand::Gauge(command)) =
            collect_inspiration_rule_op(&managers.gauge, &tag, 99998, &cards).unwrap()
        else {
            panic!("gauge command");
        };
        managers.execute_gauge(command).unwrap();

        let RuleOp::Command(BattleCommand::Gauge(command)) =
            reset_inspiration_rule_op(&managers.gauge, &tag, 99998).unwrap()
        else {
            panic!("gauge command");
        };
        managers.execute_gauge(command).unwrap();

        assert_eq!(managers.gauge.get(team_energy_key(1)).unwrap().current, 2);
        assert_eq!(
            managers.gauge.get(inspiration_key(99998)).unwrap().current,
            0
        );
    }

    #[test]
    fn finalization_snapshots_only_after_inspiration_participates() {
        crate::test_support::init_config();
        let fight = sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    team_type: Some(1),
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(20),
                        buff_id: Some(2_240_000),
                        from_uid: Some(10),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let key = inspiration_key(99_998);
        let origin = buff_act::configured_command_origin(875, BuffActKind::EmitterTag).unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                key,
                GaugeOperation::Enable { max: None },
            ))
            .unwrap();
        managers.begin_round();

        let idle = finalize_action_queue_rule_ops(&managers, 1, 99_998);
        assert!(matches!(
            idle.as_slice(),
            [
                RuleOp::Command(BattleCommand::Card(CardCommand::ClearEnergy { .. })),
                RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                    operation: GaugeOperation::Snapshot,
                    ..
                }))
            ]
        ));

        for delta in [1, -1] {
            managers
                .execute_gauge(GaugeCommand::new(
                    origin,
                    key,
                    GaugeOperation::ChangeValue { delta },
                ))
                .unwrap();
        }
        let participated = finalize_action_queue_rule_ops(&managers, 1, 99_998);
        assert!(matches!(
            participated.as_slice(),
            [
                RuleOp::Command(BattleCommand::Card(CardCommand::ClearEnergy { .. })),
                RuleOp::Command(BattleCommand::Gauge(GaugeCommand {
                    operation: GaugeOperation::Snapshot,
                    ..
                }))
            ]
        ));
    }

    #[test]
    fn inputs_fail_closed_without_the_exact_feature_or_enabled_gauge() {
        let tag = feature(10, 1, 20, "EmitterTag", vec![875]);
        let wrong = feature(10, 1, 21, "EmitterTag", vec![875]);
        let managers = BattleManagers::default();

        assert!(team_energy_gain_rule_op(&managers.gauge, &wrong, 2).is_none());
        assert!(collect_inspiration_rule_op(&managers.gauge, &tag, 99998, &[]).is_none());
        assert!(reset_inspiration_rule_op(&managers.gauge, &tag, 99998).is_none());
    }

    fn feature(
        owner_uid: i64,
        team_type: i32,
        buff_uid: i64,
        act_type: &str,
        values: Vec<i32>,
    ) -> ActiveBuffFeature {
        ActiveBuffFeature {
            owner_uid,
            source_uid: owner_uid,
            buff_uid,
            buff_id: 2240008,
            amount: 1,
            team_type,
            owner_alive: true,
            act_type: act_type.to_owned(),
            effect_time: 0,
            effect_condition: 0,
            raw: values
                .iter()
                .map(i32::to_string)
                .collect::<Vec<_>>()
                .join("#"),
            values,
        }
    }
}
