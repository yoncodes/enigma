use crate::engine::skill::{buff_act, rule::RuleReferences};

use super::{BuffActDefinition, BuffActDestination, BuffActKind, find, linked_rule_ops};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffActParseError {
    EmptyArgument { cell: usize, item: usize },
    InvalidInteger { cell: usize, item: usize },
    InvalidStructuredArguments,
}

#[derive(Debug, Clone)]
pub struct ParsedBuffAct {
    pub raw: String,
    pub raw_cells: Vec<String>,
    pub raw_args: Vec<String>,
    pub act_id: Option<i32>,
    /// The complete integer projection, including the act id. This is empty
    /// when any generic cell fails to parse; callers must not use a compacted
    /// prefix as a fallback.
    pub values: Vec<i32>,
    pub args: Vec<i32>,
    pub act_type: String,
    pub effect_time: i32,
    pub effect_condition: i32,
    pub kind: Option<BuffActKind>,
    pub arguments_supported: bool,
    pub parse_error: Option<BuffActParseError>,
    pub definition: Option<&'static BuffActDefinition>,
}

impl ParsedBuffAct {
    pub fn is_malformed(&self) -> bool {
        self.parse_error.is_some()
    }

    pub fn wire(&self) -> Option<&'static buff_act::wire::BuffActWireDefinition> {
        self.definition?
            .wire
            .as_ref()
            .filter(|wire| wire.applies_to_feature(self.effect_time, self.arguments_supported))
    }

    pub fn references(&self, game: Option<&config::GameDB>) -> RuleReferences {
        self.definition
            .map(|definition| (definition.references)(game, self))
            .unwrap_or_default()
    }
}

pub fn empty_references(_: Option<&config::GameDB>, _: &ParsedBuffAct) -> RuleReferences {
    RuleReferences::default()
}

pub(super) fn references_for_feature(
    game: Option<&config::GameDB>,
    feature: &ParsedBuffAct,
) -> RuleReferences {
    if feature.is_malformed() {
        return RuleReferences::default();
    }
    let mut references = RuleReferences::default();
    let add_skill = |references: &mut RuleReferences, skill_id: Option<i32>| {
        if skill_id.is_some_and(|skill_id| skill_id > 0) {
            references.skills.extend(skill_id);
        }
    };
    let add_buff = |references: &mut RuleReferences, buff_id: Option<i32>| {
        if buff_id.is_some_and(|buff_id| buff_id > 0) {
            references.buffs.extend(buff_id);
        }
    };
    match feature.kind {
        Some(BuffActKind::GuardBreakReward) => {
            add_buff(&mut references, feature.args.first().copied());
        }
        Some(BuffActKind::HedoneCard) => {
            add_skill(&mut references, feature.args.first().copied());
            if feature.args.get(3) == Some(&1) {
                add_buff(&mut references, feature.args.get(2).copied());
            }
        }
        Some(BuffActKind::SovereignCardUnlock) => {
            add_skill(&mut references, feature.args.first().copied());
        }
        Some(BuffActKind::SovereignCardGeneration) => {
            add_skill(&mut references, feature.args.first().copied());
            add_buff(&mut references, feature.args.get(2).copied());
        }
        Some(BuffActKind::SubBuff) => add_buff(&mut references, feature.values.get(1).copied()),
        Some(BuffActKind::MasterHalo) => add_buff(&mut references, feature.values.get(2).copied()),
        Some(BuffActKind::BuffReplace) => add_buff(&mut references, feature.values.get(2).copied()),
        Some(BuffActKind::AddBuffToEnter) => add_buff(
            &mut references,
            buff_act::add_buff_to_enter::referenced_buff(&feature.args),
        ),
        Some(BuffActKind::TransferEnergyBuff) => add_buff(
            &mut references,
            buff_act::transfer_energy_buff::referenced_buff(&feature.args),
        ),
        Some(BuffActKind::ConsumeCurBuffAndAuraAddBuff) => add_buff(
            &mut references,
            buff_act::consume_and_grant::referenced_buff(&feature.args),
        ),
        Some(BuffActKind::ConsumeBuffLayerCastChannel) => {
            add_buff(
                &mut references,
                buff_act::consume_buff_layer_cast_channel::referenced_buff(&feature.args),
            );
            references.skills.extend(
                buff_act::consume_buff_layer_cast_channel::referenced_skills(&feature.args),
            );
        }
        Some(BuffActKind::AddPassiveSkills)
        | Some(BuffActKind::AddSpTempCard)
        | Some(BuffActKind::CastChannel)
        | Some(BuffActKind::ChangeEmitterSkill)
        | Some(BuffActKind::CountContinueChannel)
        | Some(BuffActKind::SpecialCountCastChannel) => {
            add_skill(&mut references, feature.values.get(1).copied())
        }
        Some(BuffActKind::AddCardCastChannel) => add_skill(
            &mut references,
            buff_act::add_card_cast_channel::referenced_skill(&feature.args),
        ),
        Some(BuffActKind::ContractCastChannel) => {
            add_buff(
                &mut references,
                buff_act::contract_cast_channel::referenced_buff(&feature.args),
            );
            add_skill(
                &mut references,
                buff_act::contract_cast_channel::referenced_skill(&feature.args),
            );
        }
        Some(BuffActKind::BeatBack) => {
            add_skill(
                &mut references,
                buff_act::riposte::holder_skill(&feature.args),
            );
            add_buff(
                &mut references,
                buff_act::riposte::holder_consumed_buff(&feature.args),
            );
        }
        Some(BuffActKind::BeatBackByCounter) => add_skill(
            &mut references,
            buff_act::riposte::counter_skill(&feature.args),
        ),
        Some(BuffActKind::CardNotCalSize) => {
            references
                .skills
                .extend(feature.values.iter().skip(1).copied().filter(|skill_id| {
                    *skill_id > 0 && game.is_some_and(|game| game.skill.get(*skill_id).is_some())
                }))
        }
        Some(BuffActKind::AdrenalineAddCard) => {
            if let Some(raw) = feature.raw_cells.get(2)
                && let Ok(skills) = raw
                    .split(',')
                    .map(|value| value.trim().parse::<i32>())
                    .collect::<Result<Vec<_>, _>>()
            {
                references
                    .skills
                    .extend(skills.into_iter().filter(|skill_id| *skill_id > 0));
            }
        }
        Some(BuffActKind::NuoDiKaCastChannel) => {
            references
                .skills
                .extend(buff_act::nuo_di_ka_cast_channel::referenced_skills(
                    &feature.args,
                ))
        }
        Some(BuffActKind::HeatScaleUseSkill) => {
            references
                .skills
                .extend(crate::engine::mechanic::heat_scale::referenced_skills(
                    &feature.raw,
                ))
        }
        Some(BuffActKind::UseSkillByElectricLvContinueChannel) => {
            references.skills.extend(
                buff_act::electric_channel::skills(&feature.args)
                    .into_iter()
                    .flatten()
                    .copied(),
            );
        }
        Some(BuffActKind::PaperCircleContinueChannel) => add_skill(
            &mut references,
            buff_act::paper_circle_continue_channel::referenced_skill(&feature.raw),
        ),
        Some(BuffActKind::BloodValueUseSkill) | Some(BuffActKind::BuffOwnedCharge) => {
            add_skill(&mut references, feature.values.get(3).copied())
        }
        Some(
            BuffActKind::UseSkillToEnemy
            | BuffActKind::ConsumeBuffContinueChannel
            | BuffActKind::ConsumeBuffAddBuffContinueChannel
            | BuffActKind::MonitorContinueChannel,
        ) => {
            if let Some(skill) = buff_act::use_skill::linked_for(
                0,
                feature.act_id.unwrap_or_default(),
                &feature.act_type,
                &feature.args,
            ) {
                add_skill(&mut references, Some(skill.skill_id));
            }
        }
        Some(BuffActKind::BeatBackDependOnAttackMe) => references.skills.extend(
            feature
                .values
                .iter()
                .skip(1)
                .take(2)
                .copied()
                .filter(|skill_id| *skill_id > 0),
        ),
        Some(BuffActKind::EmitterTag) => references.skills.extend(
            game.and_then(crate::catalog::impromptu_definition)
                .map(|definition| definition.skill_id()),
        ),
        _ => {}
    }
    references
}

pub(super) fn parse_integer_projection(
    raw_cells: &[String],
) -> Result<Vec<i32>, BuffActParseError> {
    let mut values = Vec::new();
    for (cell_index, cell) in raw_cells.iter().enumerate() {
        for (item_index, part) in cell.split(',').enumerate() {
            let part = part.trim();
            if part.is_empty() {
                return Err(BuffActParseError::EmptyArgument {
                    cell: cell_index,
                    item: item_index,
                });
            }
            values.push(
                part.parse::<i32>()
                    .map_err(|_| BuffActParseError::InvalidInteger {
                        cell: cell_index,
                        item: item_index,
                    })?,
            );
        }
    }
    Ok(values)
}

/// Resolve one configured feature through the exact buff-act registry.
///
/// `raw` and its hash cells are retained verbatim. The integer projection is
/// all-or-error; malformed cells never leave a compacted prefix in `values`.
pub fn resolve_feature(game: Option<&config::GameDB>, raw: &str) -> Option<ParsedBuffAct> {
    let raw_cells = raw.split('#').map(str::to_owned).collect::<Vec<_>>();
    let raw_args = raw_cells.iter().skip(1).cloned().collect::<Vec<_>>();
    let act_id = raw_cells
        .first()
        .and_then(|cell| cell.trim().parse::<i32>().ok());
    let act_id = act_id?;
    let act = match game {
        Some(game) => game.buff_act.get(act_id),
        None => config::try_get().and_then(|game| game.buff_act.get(act_id)),
    };
    let definition = act.and_then(|act| find(act.id, &act.r#type));
    let (values, parse_error) = match definition.and_then(|definition| definition.parser) {
        Some(parser) => match parser(&raw_args) {
            Some(args) => (std::iter::once(act_id).chain(args).collect(), None),
            None => (
                Vec::new(),
                Some(BuffActParseError::InvalidStructuredArguments),
            ),
        },
        None => match parse_integer_projection(&raw_cells) {
            Ok(values) => (values, None),
            Err(error) => (Vec::new(), Some(error)),
        },
    };
    let args = values.get(1..).unwrap_or_default().to_vec();
    let effect_time = act.map(|act| act.effect_time).unwrap_or_default();
    let effect_condition = act.map(|act| act.effect_condition).unwrap_or_default();
    let arguments_supported = parse_error.is_none()
        && definition.is_some_and(|definition| {
            definition.accepts_route(effect_time, effect_condition)
                && definition.validate_arguments(game, &args, Some(raw))
        });
    Some(ParsedBuffAct {
        raw: raw.to_owned(),
        raw_cells,
        raw_args,
        act_id: Some(act_id),
        values,
        args,
        act_type: act.map(|act| act.r#type.clone()).unwrap_or_default(),
        effect_time,
        effect_condition,
        kind: definition.map(|definition| definition.kind),
        arguments_supported,
        parse_error,
        definition,
    })
}

pub fn destination_for_feature(
    game: Option<&config::GameDB>,
    feature: &ParsedBuffAct,
) -> Option<BuffActDestination> {
    let definition = feature.definition?;
    if feature.is_malformed()
        || !definition.accepts_route(feature.effect_time, feature.effect_condition)
        || !definition.validate_arguments(game, &feature.args, Some(&feature.raw))
    {
        return None;
    }
    definition.destination().or_else(|| {
        linked_rule_ops(0, feature.act_id?, &feature.act_type, &feature.args)
            .is_some()
            .then_some(BuffActDestination::LinkedSkill)
    })
}
