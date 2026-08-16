use crate::engine::{
    entity::attr::AttrId,
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffActInfoMarkerResult},
        hp::{HpCommand, MaxHpAdjust},
    },
    skill::{
        buff_act::{self, registry::BuffActKind},
        rule::output::{BattleCommand, RuleOp},
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(
        args,
        [raw_attr, rate, cap]
            if matches!(
                AttrId::from_raw(*raw_attr),
                Some(
                    AttrId::Hp
                        | AttrId::Attack
                        | AttrId::RealityDef
                        | AttrId::MentalDef
                        | AttrId::CriticalTechnique
                )
            ) && *rate >= 0 && *cap >= 0
    )
}

fn snapshot_value(managers: &BattleManagers, feature: &ActiveBuffFeature) -> Option<(AttrId, i32)> {
    if !buff_act::is_kind(feature, BuffActKind::EachChangeAttrOneWay) {
        return None;
    }
    let [_, raw_attr, _, _] = feature.values.as_slice() else {
        return None;
    };
    let attr = AttrId::from_raw(*raw_attr)?;
    let value = managers
        .buff
        .fixed_attribute_value(feature.buff_uid, attr)?;
    Some((attr, value))
}

pub fn transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(ActiveBuffFeature, RuleOp)> {
    let change = match event {
        BattleEvent::BuffAdded(change) | BattleEvent::BuffRemoved(change) => change,
        _ => return Vec::new(),
    };
    let features = super::changed_features(managers, event, BuffActKind::EachChangeAttrOneWay);
    if features.is_empty() {
        return Vec::new();
    }

    if matches!(event, BattleEvent::BuffRemoved(_)) {
        let Some((feature, _)) = features
            .into_iter()
            .find(|(feature, _)| feature.values.get(1).copied() == Some(AttrId::Hp.id()))
        else {
            return Vec::new();
        };
        if change.act_value == 0 {
            return Vec::new();
        }
        let Some(origin) = buff_act::feature_command_origin(&feature) else {
            return Vec::new();
        };
        return vec![(
            feature,
            RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(MaxHpAdjust {
                origin,
                source_uid: change.source_uid,
                target_uid: change.target_uid,
                delta: -change.act_value,
            }))),
        )];
    }

    let snapshots = features
        .into_iter()
        .filter_map(|(feature, _)| {
            let (attr, value) = snapshot_value(managers, &feature)?;
            Some((feature, attr, value))
        })
        .collect::<Vec<_>>();
    if snapshots.is_empty() {
        return Vec::new();
    }
    let mut ops = Vec::new();
    for (feature, attr, value) in snapshots {
        ops.push((
            feature.clone(),
            RuleOp::BuffActInfoMarker(BuffActInfoMarkerResult {
                target_uid: 0,
                buff_uid: change.buff_uid,
                act_id: feature.act_id().unwrap_or_default(),
                params: Vec::new(),
                str_param: Some(format!("{}#{value}", attr.id())),
                team_type: 0,
            }),
        ));
        if attr == AttrId::Hp && value != 0 {
            let Some(origin) = buff_act::feature_command_origin(&feature) else {
                continue;
            };
            ops.push((
                feature,
                RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(MaxHpAdjust {
                    origin,
                    source_uid: change.source_uid,
                    target_uid: change.target_uid,
                    delta: value,
                }))),
            ));
        }
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        manager::buff::{BuffCommand, BuffGrant, BuffRemove, BuffRemoveSelector},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute};

    fn managers(source_attack: i32, source_hp: i32) -> BattleManagers {
        crate::test_support::init_config();
        BattleManagers::seeded(&Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        team_type: Some(1),
                        current_hp: Some(source_hp),
                        attr: Some(HeroAttribute {
                            hp: Some(source_hp),
                            attack: Some(source_attack),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(11),
                        team_type: Some(1),
                        current_hp: Some(2_000),
                        attr: Some(HeroAttribute {
                            hp: Some(2_000),
                            attack: Some(1_000),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    #[test]
    fn supports_only_capped_core_attribute_shape() {
        assert!(supports(&[AttrId::Attack.id(), 80, 250]));
        assert!(supports(&[AttrId::Hp.id(), 100, 1_550]));
        assert!(!supports(&[AttrId::Attack.id(), 80]));
        assert!(!supports(&[AttrId::DmgBonus.id(), 80, 250]));
    }

    fn grant(managers: &mut BattleManagers, buff_id: i32) -> BattleEvent {
        let changes = managers
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1131, "EachChangeAttrOneWay"),
                },
                source_uid: 10,
                target_uid: 11,
                buff_id,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap();
        let added = changes.change.added.as_ref().expect("buff added");
        assert_eq!(
            added
                .buff
                .act_info
                .iter()
                .filter_map(|info| info.str_param.as_deref())
                .collect::<Vec<_>>(),
            if buff_id == 31460131 {
                vec!["102#250", "101#1250"]
            } else {
                vec!["102#310", "101#1550"]
            }
        );
        changes
            .events()
            .into_iter()
            .find(|event| matches!(event, BattleEvent::BuffAdded(_)))
            .unwrap()
    }

    #[test]
    fn snapshots_source_relative_values_and_caps() {
        let mut managers = managers(4_000, 20_000);
        let event = grant(&mut managers, 31460131);
        let ops = transaction_rule_ops(&managers, &event);
        let markers = ops
            .iter()
            .filter_map(|(_, op)| match op {
                RuleOp::BuffActInfoMarker(marker) => marker.str_param.clone(),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(markers, ["102#250", "101#1250"]);
        assert!(ops.iter().any(|(_, op)| matches!(
            op,
            RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(change)))
                if change.delta == 1_250
        )));
        assert_eq!(managers.origin_attribute(11, AttrId::Attack), 1_250);
    }

    #[test]
    fn rejected_attribute_shape_creates_no_snapshot_or_contribution() {
        let mut managers = managers(4_000, 20_000);
        let changes = managers
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1131, "EachChangeAttrOneWay"),
                },
                source_uid: 10,
                target_uid: 11,
                buff_id: 31440111,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap();
        let added = changes.change.added.as_ref().expect("buff added");

        assert!(
            added
                .buff
                .act_info
                .iter()
                .all(|info| info.act_id != Some(1131))
        );
        assert_eq!(
            managers.buff.fixed_attribute_delta(11, AttrId::CriticalDmg),
            0
        );
    }

    #[test]
    fn enhanced_caps_and_removal_use_the_snapshot() {
        let mut managers = managers(4_000, 20_000);
        let added = grant(&mut managers, 31460139);
        let BattleEvent::BuffAdded(change) = added else {
            unreachable!()
        };
        let removal = managers
            .execute_buff(BuffCommand::Remove(BuffRemove {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(1131, "EachChangeAttrOneWay"),
                },
                target_uid: 11,
                selector: BuffRemoveSelector::Uid(change.buff_uid),
            }))
            .unwrap();
        let removed = removal
            .events()
            .into_iter()
            .find(|event| matches!(event, BattleEvent::BuffRemoved(_)))
            .unwrap();
        assert!(matches!(
            removed,
            BattleEvent::BuffRemoved(change) if change.act_value == 1_550
        ));
        let ops = transaction_rule_ops(&managers, &removed);
        assert!(ops.iter().any(|(_, op)| matches!(
            op,
            RuleOp::Command(BattleCommand::Hp(HpCommand::AdjustMax(change)))
                if change.delta == -1_550
        )));
    }
}
