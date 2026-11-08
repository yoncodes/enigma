use std::collections::{HashMap, HashSet};

use database::{
    db::game::rouge,
    models::game::{achievements::Achievement, rouge::Rouge2MaterialState},
};
use sonettobuf::{
    Rouge2AlchemyInfo, Rouge2AlchemyMaterialInfo, Rouge2AlchemyReply, Rouge2CurAlchemyInfo,
};
use sqlx::SqlitePool;

use crate::error::AppError;

const EMPTY_STATE: i32 = 0;
const ENDED_STATE: i32 = 3;
const SUB_MATERIAL_COUNT: i32 = 115;
const SUPPORTED_SUB_MATERIALS: usize = 1;
const GENIUS_POINT_GAIN: i32 = 1;
const CERTAIN_EFFECT_CHANCE: i32 = 100;

pub struct AlchemyResult {
    pub reply: Rouge2AlchemyReply,
    pub genius_point: i32,
    pub achievements: Vec<Achievement>,
}

pub async fn combine(
    db: &SqlitePool,
    user_id: i64,
    formula_id: i32,
    sub_materials: Vec<i32>,
) -> Result<AlchemyResult, AppError> {
    let tables = config::configs::get();
    let before = rouge::get_or_create_rouge2_outside(db, user_id, tables).await?;
    if !matches!(before.state.state, EMPTY_STATE | ENDED_STATE) || before.state.alchemy_formula != 0
    {
        return Err(AppError::InvalidRequest);
    }

    let formula = tables
        .rouge2_formula
        .get(formula_id)
        .filter(|formula| {
            formula.is_hide == 0 && formula.condition.is_empty() && formula.score_reward == 0
        })
        .ok_or(AppError::InvalidRequest)?;
    let main_effect = formula
        .effects
        .parse::<i32>()
        .ok()
        .filter(|effect| *effect > 0)
        .ok_or_else(|| AppError::Custom(format!("invalid Rouge2 formula effect {formula_id}")))?;
    let mut costs = parse_main_costs(formula_id, &formula.main_id_num, tables)?;
    let max_sub_materials = tables
        .rouge2_outside_const
        .get(SUB_MATERIAL_COUNT)
        .and_then(|row| row.value.parse::<usize>().ok())
        .ok_or_else(|| AppError::Custom("invalid Rouge2 sub-material limit".into()))?;
    if sub_materials.len() != SUPPORTED_SUB_MATERIALS || sub_materials.len() > max_sub_materials {
        return Err(AppError::InvalidRequest);
    }

    let mut selected = HashSet::new();
    let mut sub_effects = Vec::with_capacity(sub_materials.len());
    for material_id in sub_materials {
        if !selected.insert(material_id) {
            return Err(AppError::InvalidRequest);
        }
        let material = tables
            .rouge2_material
            .get(material_id)
            .filter(|material| material.r#type == 2)
            .ok_or(AppError::InvalidRequest)?;
        sub_effects.push(parse_sub_effect(material_id, &material.effects)?);
        costs.push((material_id, 1));
    }

    require_materials(&before.materials, &costs)?;
    let sub_effects_json = serde_json::to_string(&sub_effects)?;
    let applied = rouge::apply_alchemy(
        db,
        user_id,
        rouge::AlchemyUpdate {
            expected_state: before.state.state,
            formula: formula_id,
            main_effect,
            sub_effects: &sub_effects_json,
            genius_point_gain: GENIUS_POINT_GAIN,
            costs: &costs,
        },
    )
    .await?
    .ok_or(AppError::InvalidRequest)?;

    Ok(result(applied, formula_id, main_effect, sub_effects))
}

fn parse_main_costs(
    formula_id: i32,
    value: &str,
    tables: &config::GameDB,
) -> Result<Vec<(i32, i32)>, AppError> {
    let mut seen = HashSet::new();
    value
        .split('|')
        .map(|entry| {
            let (material, amount) = entry.split_once('#').ok_or_else(|| {
                AppError::Custom(format!("invalid Rouge2 formula material {formula_id}"))
            })?;
            let material_id = material.parse::<i32>().ok().filter(|id| *id > 0);
            let amount = amount.parse::<i32>().ok().filter(|amount| *amount > 0);
            let (material_id, amount) = material_id.zip(amount).ok_or_else(|| {
                AppError::Custom(format!("invalid Rouge2 formula material {formula_id}"))
            })?;
            let material = tables.rouge2_material.get(material_id).ok_or_else(|| {
                AppError::Custom(format!("missing Rouge2 material {material_id}"))
            })?;
            let belongs_to_formula = material
                .formula
                .split('#')
                .any(|id| id.parse::<i32>() == Ok(formula_id));
            if material.r#type != 1 || !belongs_to_formula || !seen.insert(material_id) {
                return Err(AppError::Custom(format!(
                    "invalid Rouge2 formula material {material_id}"
                )));
            }
            Ok((material_id, amount))
        })
        .collect()
}

fn parse_sub_effect(material_id: i32, value: &str) -> Result<i32, AppError> {
    let (effect, chance) = value
        .split_once('#')
        .ok_or_else(|| AppError::Custom(format!("invalid Rouge2 material effect {material_id}")))?;
    match (effect.parse::<i32>(), chance.parse::<i32>()) {
        (Ok(effect), Ok(CERTAIN_EFFECT_CHANCE)) if effect > 0 => Ok(effect),
        _ => Err(AppError::Custom(format!(
            "unsupported Rouge2 material effect {material_id}"
        ))),
    }
}

fn require_materials(
    materials: &[Rouge2MaterialState],
    costs: &[(i32, i32)],
) -> Result<(), AppError> {
    let available = materials
        .iter()
        .map(|material| (material.material_id, material.num))
        .collect::<HashMap<_, _>>();
    if costs
        .iter()
        .any(|(id, amount)| available.get(id).is_none_or(|available| available < amount))
    {
        return Err(AppError::InsufficientItems);
    }
    Ok(())
}

fn result(
    applied: rouge::AlchemyApply,
    formula: i32,
    main_effect: i32,
    sub_effects: Vec<i32>,
) -> AlchemyResult {
    AlchemyResult {
        genius_point: applied.genius_point,
        achievements: applied.achievements,
        reply: Rouge2AlchemyReply {
            alchemy_info: Some(Rouge2AlchemyInfo {
                cur_alchemy_info: Some(Rouge2CurAlchemyInfo {
                    formula: Some(formula),
                    main_effect: Some(main_effect),
                    sub_effect: sub_effects,
                }),
                alchemy_material_info: applied
                    .materials
                    .into_iter()
                    .map(|material| Rouge2AlchemyMaterialInfo {
                        id: Some(material.material_id),
                        num: Some(material.num),
                    })
                    .collect(),
            }),
            sp_event_num: Some(0),
            return_material: Vec::new(),
            sub_extra_effect: Vec::new(),
            main_update_effect: None,
        },
    }
}
