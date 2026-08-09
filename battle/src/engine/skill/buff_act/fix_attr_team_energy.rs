use crate::engine::{
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffManager, BuffSetState},
    },
    skill::{
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
        target::TargetPool,
    },
};

use super::{feature_kind, registry::BuffActKind};

pub fn supports(kind: BuffActKind, args: &[i32]) -> bool {
    match kind {
        BuffActKind::FixAttrTeamEnergy => {
            matches!(args, [attr_id, _, _] if AttrId::from_raw(*attr_id).is_some())
        }
        BuffActKind::FixAttrTeamEnergyAndBuff => {
            matches!(args, [attr_id, _, _, _, buff_id] if AttrId::from_raw(*attr_id).is_some() && *buff_id > 0)
        }
        _ => false,
    }
}

pub fn grant_params(
    managers: &BattleManagers,
    pool: &TargetPool,
    buff_id: i32,
    target_uid: i64,
    team_type: i32,
) -> Option<String> {
    let feature = managers
        .buff
        .definition_features(buff_id)
        .into_iter()
        .find(|feature| {
            matches!(
                feature_kind(feature),
                Some(BuffActKind::FixAttrTeamEnergy | BuffActKind::FixAttrTeamEnergyAndBuff)
            )
        })?;
    let act_id = *feature.values.first()?;
    let energy = managers
        .gauge
        .get(crate::engine::mechanic::impromptu::team_energy_key(
            team_type,
        ))?
        .current;
    let value = match feature_kind(&feature)? {
        BuffActKind::FixAttrTeamEnergy => energy.max(0),
        BuffActKind::FixAttrTeamEnergyAndBuff => team_energy_and_holder_value(
            &feature.values[1..],
            target_uid,
            energy,
            &managers.buff,
            pool,
        )?,
        _ => return None,
    };
    Some(format!("{act_id}#{value}"))
}

pub fn rule_ops(
    managers: &BattleManagers,
    pool: &TargetPool,
    subscriber: &BuffActSubscriber,
    event: &crate::engine::event::payload::BattleEvent,
) -> Option<Vec<RuleOp>> {
    let kind = super::subscriber_kind(subscriber)?;
    if kind != BuffActKind::FixAttrTeamEnergyAndBuff || !supports(kind, &subscriber.args) {
        return None;
    }
    if !matches!(
        event,
        crate::engine::event::payload::BattleEvent::RoundStart
    ) {
        return Some(Vec::new());
    }
    let inspiration = managers
        .gauge
        .get(crate::engine::mechanic::impromptu::team_energy_key(
            subscriber.team_type,
        ))
        .map(|state| state.current)
        .unwrap_or_default();
    let value = team_energy_and_holder_value(
        &subscriber.args,
        subscriber.owner_uid,
        inspiration,
        &managers.buff,
        pool,
    )?;
    Some(vec![RuleOp::Command(BattleCommand::Buff(
        BuffCommand::SetState(BuffSetState {
            ex_info: None,
            origin: super::command_origin(subscriber)
                .filter(|_| super::subscriber_is_kind(subscriber, kind))?,
            target_uid: subscriber.owner_uid,
            buff_uid: subscriber.buff_uid,
            params: Some(format!("{}#{value}", subscriber.key.definition.opcode)),
            act_info: None,
        }),
    ))])
}

pub fn attribute_delta(
    feature: &ActiveBuffFeature,
    attr_id: AttrId,
    inspiration: i32,
    buffs: &BuffManager,
    pool: &TargetPool,
) -> i32 {
    let [_, raw_attr_id, base, per_energy, ..] = feature.values.as_slice() else {
        return 0;
    };
    if AttrId::from_raw(*raw_attr_id) != Some(attr_id) {
        return 0;
    }
    let stored_value = buffs
        .active_for(feature.owner_uid)
        .find(|buff| buff.uid == Some(feature.buff_uid))
        .and_then(|buff| buff.act_common_params.as_deref())
        .and_then(|params| {
            let mut parts = params.split('#');
            let act_id = parts.next()?.parse::<i32>().ok()?;
            (Some(&act_id) == feature.values.first())
                .then(|| parts.next()?.parse::<i32>().ok())
                .flatten()
        });
    match feature_kind(feature) {
        Some(BuffActKind::FixAttrTeamEnergy) => {
            base + per_energy * stored_value.unwrap_or(inspiration).max(0)
        }
        Some(BuffActKind::FixAttrTeamEnergyAndBuff) => stored_value.unwrap_or_else(|| {
            team_energy_and_holder_value(
                &feature.values[1..],
                feature.owner_uid,
                inspiration,
                buffs,
                pool,
            )
            .unwrap_or_default()
        }),
        _ => 0,
    }
}

fn team_energy_and_holder_value(
    args: &[i32],
    owner_uid: i64,
    inspiration: i32,
    buffs: &BuffManager,
    pool: &TargetPool,
) -> Option<i32> {
    let [_, base, per_energy, per_holder, holder_buff_id] = args else {
        return None;
    };
    let holders = pool
        .main_allies(owner_uid)
        .iter()
        .filter(|ally| ally.uid != owner_uid)
        .filter(|ally| buffs.has_active_buff_id_or_type(ally.uid, *holder_buff_id))
        .count() as i64;
    let value = (i64::from(*base) + i64::from(*per_energy) * i64::from(inspiration.max(0)))
        * (1000 + i64::from(*per_holder) * holders)
        / 1000;
    Some(value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32)
}

#[cfg(test)]
mod tests {
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        manager::gauge::{GaugeCommand, GaugeOperation},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    use super::*;

    #[test]
    fn team_inspiration_attribute_uses_the_configured_base_and_rate() {
        let feature = ActiveBuffFeature {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 1,
            buff_id: 31080143,
            amount: 1,
            team_type: 1,
            owner_alive: true,
            act_type: "FixAttrTeamEnergy".into(),
            effect_time: 0,
            effect_condition: 0,
            raw: "882#205#50#25".into(),
            values: vec![882, 205, 50, 25],
        };

        assert_eq!(
            attribute_delta(
                &feature,
                AttrId::DmgBonus,
                6,
                &BuffManager::default(),
                &TargetPool::default(),
            ),
            200
        );
    }

    #[test]
    fn holder_scaled_variant_reads_the_committed_attribute_snapshot_once() {
        crate::test_support::init_config();
        let fight = sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                entitys: vec![sonettobuf::FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![sonettobuf::BuffInfo {
                        buff_id: Some(31080131),
                        uid: Some(1),
                        from_uid: Some(10),
                        act_common_params: Some("883#340".to_owned()),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let pool = TargetPool::from_fight(&fight);
        let managers = BattleManagers::seeded(&fight);
        let feature = managers
            .buff
            .active_features(&managers.hp)
            .into_iter()
            .find(|feature| feature_kind(feature) == Some(BuffActKind::FixAttrTeamEnergyAndBuff))
            .unwrap();

        assert_eq!(
            attribute_delta(&feature, AttrId::Attack, 999, &managers.buff, &pool,),
            340
        );
    }

    #[test]
    fn round_start_rule_snapshots_the_owned_team_energy_gauge() {
        let mut managers = BattleManagers::default();
        let key = crate::engine::mechanic::impromptu::team_energy_key(1);
        let origin = CommandOrigin {
            domain: RuleDomain::Lifecycle,
            key: DefinitionKey::new(0, "Test"),
        };
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                key,
                GaugeOperation::Enable { max: None },
            ))
            .unwrap();
        let emitter_energy = crate::engine::mechanic::impromptu::inspiration_key(
            crate::engine::manager::emitter::UID,
        );
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                emitter_energy,
                GaugeOperation::Enable { max: None },
            ))
            .unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                emitter_energy,
                GaugeOperation::ChangeValue { delta: 99 },
            ))
            .unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                key,
                GaugeOperation::ChangeValue { delta: 7 },
            ))
            .unwrap();
        let subscriber = BuffActSubscriber {
            owner_uid: 10,
            source_uid: 10,
            buff_uid: 1,
            buff_id: 31080131,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::RoundStart,
                DefinitionKey::new(883, "FixAttrTeamEnergyAndBuff"),
            ),
            act_type: "FixAttrTeamEnergyAndBuff".into(),
            effect_time: 104,
            effect_condition: 0,
            args: vec![102, 300, 5, 150, 31080131],
            raw: "883#102#300#5#150#31080131".into(),
        };

        let ops = rule_ops(
            &managers,
            &TargetPool::default(),
            &subscriber,
            &crate::engine::event::payload::BattleEvent::RoundStart,
        )
        .unwrap();

        assert!(matches!(
            ops.as_slice(),
            [RuleOp::Command(BattleCommand::Buff(BuffCommand::SetState(
                BuffSetState { params: Some(params), .. }
            )))] if params == "883#335"
        ));
    }

    #[test]
    fn each_team_energy_variant_snapshots_its_own_encoded_value_on_grant() {
        crate::test_support::init_config();
        let mut managers = BattleManagers::default();
        let origin = CommandOrigin {
            domain: RuleDomain::Lifecycle,
            key: DefinitionKey::new(0, "Test"),
        };
        let key = crate::engine::mechanic::impromptu::team_energy_key(1);
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                key,
                GaugeOperation::Enable { max: None },
            ))
            .unwrap();
        managers
            .execute_gauge(GaugeCommand::new(
                origin,
                key,
                GaugeOperation::ChangeValue { delta: 10 },
            ))
            .unwrap();

        assert_eq!(
            grant_params(&managers, &TargetPool::default(), 31080143, 10, 1).as_deref(),
            Some("882#10")
        );
        assert_eq!(
            grant_params(&managers, &TargetPool::default(), 31080131, 10, 1).as_deref(),
            Some("883#350")
        );
    }
}
