use std::borrow::Cow;

use sonettobuf::{Fight, FightEntityInfo, FightRound, FightStep, effect_type_enum::EffectType};

pub(super) fn before_deal<'a>(fight: &'a Fight, round: &FightRound) -> Cow<'a, Fight> {
    if fight.version != Some(7) || fight.attacker.is_none() {
        return Cow::Borrowed(fight);
    }
    let mut view = fight.clone();
    let entities = &mut view.attacker.as_mut().unwrap().entitys;
    if read_until_deal(entities, &round.fight_step) {
        Cow::Owned(view)
    } else {
        Cow::Borrowed(fight)
    }
}

fn read_until_deal(entities: &mut [FightEntityInfo], steps: &[FightStep]) -> bool {
    for effect in steps.iter().flat_map(|step| &step.act_effect) {
        let kind = effect
            .effect_type
            .and_then(|kind| EffectType::try_from(kind).ok());
        if kind == Some(EffectType::Enterfightdeal) {
            return true;
        }
        if let Some(target_uid) = effect.target_id
            && let Some(buff) = effect.buff.as_ref()
            && buff.uid.is_some()
            && let Some(entity) = entities
                .iter_mut()
                .find(|entity| entity.uid == Some(target_uid))
        {
            match kind {
                Some(EffectType::Buffadd) => {
                    entity.buffs.retain(|active| active.uid != buff.uid);
                    entity.buffs.push(buff.clone());
                }
                Some(EffectType::Buffdel) => entity.buffs.retain(|active| active.uid != buff.uid),
                _ => {}
            }
        }
        if let Some(nested) = effect.fight_step.as_ref()
            && read_until_deal(entities, std::slice::from_ref(nested))
        {
            return true;
        }
    }
    false
}
