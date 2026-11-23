use crate::engine::{
    entity::attr::AttrId,
    manager::{
        buff::{ActiveBuffFeature, BuffManager},
        hp::HpManager,
    },
    skill::{buff_act::registry, rule::DefinitionKey},
};

use super::Steps;

const KEY: DefinitionKey = DefinitionKey::new(853, "AttrByLostHp");
const ATTRIBUTE: AttrId = AttrId::PlaymodeDmgIncrease;

#[derive(Debug, Clone, Default)]
pub(crate) struct OutgoingEvaluation {
    source_uid: i64,
    rate: i32,
}

impl OutgoingEvaluation {
    pub(crate) fn begin(&mut self, source_uid: i64, buffs: &BuffManager, hp: &HpManager) {
        let health = hp.get(source_uid);
        self.source_uid = source_uid;
        self.rate = buffs
            .active_features(hp)
            .iter()
            .filter(|feature| feature.owner_uid == source_uid)
            .filter_map(outgoing_steps)
            .map(|steps| steps.delta(ATTRIBUTE, health))
            .sum();
    }

    pub(crate) fn rate(&self, uid: i64) -> i32 {
        if self.source_uid == uid { self.rate } else { 0 }
    }

    pub(crate) fn applies(feature: &ActiveBuffFeature) -> bool {
        outgoing_steps(feature).is_some()
    }
}

fn outgoing_steps(feature: &ActiveBuffFeature) -> Option<Steps<'_>> {
    let definition = registry::find(feature.act_id()?, &feature.act_type)?;
    if definition.key != KEY {
        return None;
    }
    let steps = Steps::parse(&feature.raw)?;
    (steps.absolute_mode == Some("0")
        && steps.attrs.parse().ok().and_then(AttrId::from_raw) == Some(ATTRIBUTE))
    .then_some(steps)
}
