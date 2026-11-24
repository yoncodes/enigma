use std::{
    collections::HashMap,
    sync::{Arc, OnceLock, RwLock},
};

use sonettobuf::BuffInfo;

use crate::{
    catalog::BattleCatalog,
    engine::{manager::buff::BuffStatus, skill::buff_act::registry::BuffActKind},
};

#[derive(Debug, Clone)]
pub(super) struct TargetBuff {
    pub(super) id: i32,
    pub(super) type_id: i32,
    pub(super) source_uid: i64,
    definition: Arc<TargetBuffDefinition>,
}

#[derive(Debug)]
struct TargetBuffDefinition {
    type_id: i32,
    status: Option<BuffStatus>,
    features: Vec<&'static str>,
    act_kinds: Vec<BuffActKind>,
    monster_labels: Vec<i32>,
}

impl PartialEq for TargetBuff {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.type_id == other.type_id
            && self.source_uid == other.source_uid
            && self.definition.status == other.definition.status
            && self.definition.features == other.definition.features
            && self.definition.act_kinds == other.definition.act_kinds
            && self.definition.monster_labels == other.definition.monster_labels
    }
}

impl Eq for TargetBuff {}

impl TargetBuff {
    pub(super) fn from_buff_info(catalog: BattleCatalog, buff: &BuffInfo) -> Self {
        let id = buff.buff_id.unwrap_or_default();
        let definition = TargetBuffDefinition::get(catalog, id);
        Self {
            id,
            type_id: buff.r#type.unwrap_or(definition.type_id),
            source_uid: buff.from_uid.unwrap_or_default(),
            definition,
        }
    }

    pub(super) fn status(&self) -> Option<BuffStatus> {
        self.definition.status
    }

    pub(super) fn blocks_enemy_aim(&self, catalog: BattleCatalog) -> bool {
        self.definition.features.iter().any(|feature| {
            feature
                .split('#')
                .next()
                .and_then(|opcode| opcode.parse().ok())
                .and_then(|opcode| catalog.buff_act_name(opcode))
                .is_some_and(|name| matches!(name, "CantSelect" | "CantSelectEx"))
        })
    }

    pub(super) fn has_buff_act_kind(&self, kind: BuffActKind) -> bool {
        self.definition.act_kinds.contains(&kind)
    }

    pub(super) fn has_monster_label(&self, label: i32) -> bool {
        self.definition.monster_labels.contains(&label)
    }
}

type Definitions = HashMap<(usize, i32), Arc<TargetBuffDefinition>>;

impl TargetBuffDefinition {
    fn get(catalog: BattleCatalog, id: i32) -> Arc<Self> {
        static DEFINITIONS: OnceLock<RwLock<Definitions>> = OnceLock::new();
        let definitions = DEFINITIONS.get_or_init(RwLock::default);
        let key = (std::ptr::from_ref(catalog.game_data()).addr(), id);
        if let Some(definition) = definitions.read().unwrap().get(&key).cloned() {
            return definition;
        }
        let definition = Arc::new(Self::new(catalog, id));
        if catalog.game_data().skill_buff.get(id).is_none() {
            return definition;
        }
        definitions
            .write()
            .unwrap()
            .entry(key)
            .or_insert(definition)
            .clone()
    }

    fn new(catalog: BattleCatalog, id: i32) -> Self {
        let features: Vec<_> = catalog
            .buff_feature_rows(id)
            .into_iter()
            .map(str::trim)
            .filter(|feature| !feature.is_empty())
            .collect();
        let act_kinds = features
            .iter()
            .filter_map(|feature| {
                let opcode = feature.split('#').next()?.parse().ok()?;
                Some(catalog.buff_act_definition(opcode)?.kind)
            })
            .collect();
        let monster_labels = features
            .iter()
            .filter_map(|feature| {
                let mut values = feature
                    .split('#')
                    .filter_map(|value| value.parse::<i32>().ok());
                let opcode = values.next()?;
                (catalog.buff_act_definition(opcode)?.kind == BuffActKind::MonsterLabel)
                    .then(|| values.next())?
            })
            .collect();
        Self {
            type_id: catalog.buff_type_id(id),
            status: catalog.buff_status(id),
            features,
            act_kinds,
            monster_labels,
        }
    }
}

#[cfg(test)]
mod tests;
