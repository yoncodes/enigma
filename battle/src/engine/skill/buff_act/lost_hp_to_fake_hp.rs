use sonettobuf::BuffActInfo;

use crate::engine::{
    damage::scale_permille,
    entity::attr::AttrId,
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffActInfoMarkerResult, BuffCommand, BuffSetState},
    },
    skill::buff_act::registry::BuffActKind,
};

#[derive(Clone, Copy)]
pub(crate) struct FakeHpState {
    pub buff_uid: i64,
    pub act_id: i32,
    pub current: i32,
    pub maximum: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FakeHpStateError {
    Multiple,
    Invalid,
}

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [raw_attr, bonus] if AttrId::from_raw(*raw_attr) == Some(AttrId::Attack) && *bonus > 0)
}

pub fn initial_act_info(source_attack: i32, values: &[i32]) -> Option<BuffActInfo> {
    let [act_id, raw_attr, bonus] = values else {
        return None;
    };
    if !supports(&[*raw_attr, *bonus]) {
        return None;
    }
    Some(BuffActInfo {
        act_id: Some(*act_id),
        param: vec![0, scale_permille(source_attack.max(0), *bonus)],
        str_param: Some(String::new()),
    })
}

pub(crate) fn state(
    managers: &BattleManagers,
    owner_uid: i64,
) -> Result<Option<FakeHpState>, FakeHpStateError> {
    let mut features = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| {
            feature.owner_uid == owner_uid && super::is_kind(feature, BuffActKind::LostHpToFakeHp)
        });
    let Some(feature) = features.next() else {
        return Ok(None);
    };
    if features.next().is_some() {
        return Err(FakeHpStateError::Multiple);
    }
    state_for_feature(managers, &feature)
        .map(Some)
        .ok_or(FakeHpStateError::Invalid)
}

pub(crate) fn damage_consumption(
    managers: &BattleManagers,
    owner_uid: i64,
) -> Result<Option<crate::engine::manager::hp::FakeHpConsumption>, FakeHpStateError> {
    Ok(
        state(managers, owner_uid)?.map(|state| crate::engine::manager::hp::FakeHpConsumption {
            buff_uid: state.buff_uid,
            consumed: 0,
        }),
    )
}

fn state_for_feature(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
) -> Option<FakeHpState> {
    if !super::is_kind(feature, BuffActKind::LostHpToFakeHp) {
        return None;
    }
    let act_id = feature.act_id()?;
    let buff = managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)?;
    let mut matching = buff
        .act_info
        .iter()
        .filter(|info| info.act_id == Some(act_id));
    let info = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    let [current, maximum] = info.param.as_slice() else {
        return None;
    };
    (info.str_param.as_deref() == Some("") && (0..=*maximum).contains(current) && *maximum > 0)
        .then_some(FakeHpState {
            buff_uid: feature.buff_uid,
            act_id,
            current: *current,
            maximum: *maximum,
        })
}

pub fn hp_loss_state_update(
    managers: &BattleManagers,
    feature: &ActiveBuffFeature,
    amount: i32,
) -> Option<Vec<super::HpLossStateUpdate>> {
    if amount <= 0 {
        return Some(Vec::new());
    }
    let state = state(managers, feature.owner_uid).ok()??;
    if state.buff_uid != feature.buff_uid {
        return None;
    }
    let maximum_hp = managers.hp.max(feature.owner_uid);
    if maximum_hp <= 0 {
        return Some(Vec::new());
    }
    let added = (i64::from(amount) * i64::from(state.maximum) / i64::from(maximum_hp))
        .clamp(0, i64::from(i32::MAX)) as i32;
    let next = state.current.saturating_add(added).min(state.maximum);
    if next == state.current {
        return Some(Vec::new());
    }
    let mut buff = managers
        .buff
        .snapshot(feature.owner_uid, feature.buff_uid)?;
    let info = buff
        .act_info
        .iter_mut()
        .find(|info| info.act_id == Some(state.act_id))?;
    info.param = vec![next, state.maximum];
    info.str_param = Some(String::new());
    let marker = BuffActInfoMarkerResult {
        target_uid: feature.owner_uid,
        buff_uid: feature.buff_uid,
        act_id: state.act_id,
        params: info.param.clone(),
        str_param: info.str_param.clone(),
        team_type: 0,
    };
    Some(vec![super::HpLossStateUpdate {
        command: BuffCommand::SetInternalState(BuffSetState {
            origin: super::feature_command_origin(feature)?,
            target_uid: feature.owner_uid,
            buff_uid: feature.buff_uid,
            ex_info: None,
            params: None,
            act_info: Some(buff.act_info),
        }),
        marker,
        after_hp: false,
    }])
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};

    use crate::engine::{
        manager::{
            BattleManagers,
            buff::{BuffCommand, BuffGrant},
            hp::{DamageEffectKind, HpCommand, HpDamage, HurtDamageFromType, HurtInfoData},
        },
        runtime::{change::BattleChange, executor::RuleOutcome},
        skill::rule::{CommandOrigin, DefinitionKey, RuleDomain},
    };

    use super::*;

    fn origin() -> CommandOrigin {
        CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(103374111, "Damage"),
        }
    }

    fn damage(target_uid: i64, amount: i32) -> HpCommand {
        HpCommand::Damage(HpDamage {
            origin: origin(),
            source_uid: 99,
            target_uid,
            amount,
            config_effect: -1,
            effect_kind: DamageEffectKind::Normal,
            assassinate: false,
            ignore_riposte: false,
            hurt: HurtInfoData {
                from_uid: 99,
                is_crit: false,
                career_restraint: false,
                reduce_hp: 0,
                effect_id: 103374111,
                skill_id: 103374111,
                damage_from: HurtDamageFromType::Skill,
                buff_act_id: 0,
                buff_uid: 0,
                hurt_effect_type: 0,
                display_amount: None,
            },
        })
    }

    fn fight(current: i32) -> Fight {
        Fight {
            attacker: Some(FightTeam {
                entitys: vec![
                    FightEntityInfo {
                        uid: Some(10),
                        team_type: Some(1),
                        current_hp: Some(1_000),
                        attr: Some(HeroAttribute {
                            hp: Some(1_000),
                            attack: Some(1_622),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    FightEntityInfo {
                        uid: Some(20),
                        team_type: Some(1),
                        current_hp: Some(10_903),
                        attr: Some(HeroAttribute {
                            hp: Some(10_903),
                            ..Default::default()
                        }),
                        buffs: vec![BuffInfo {
                            uid: Some(30),
                            buff_id: Some(31450143),
                            from_uid: Some(10),
                            layer: Some(1),
                            act_info: vec![BuffActInfo {
                                act_id: Some(1150),
                                param: vec![current, 8_108],
                                str_param: Some(String::new()),
                            }],
                            ..Default::default()
                        }],
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn feature(managers: &BattleManagers) -> ActiveBuffFeature {
        managers
            .buff
            .active_features(&managers.hp)
            .into_iter()
            .find(|feature| {
                feature.owner_uid == 20
                    && super::super::is_kind(feature, BuffActKind::LostHpToFakeHp)
            })
            .expect("Everecho Remembrance feature should be active")
    }

    #[test]
    fn initial_state_scales_the_source_attack_into_the_configured_capacity() {
        assert_eq!(
            initial_act_info(2_027, &[1150, 102, 4_000]),
            Some(BuffActInfo {
                act_id: Some(1150),
                param: vec![0, 8_108],
                str_param: Some(String::new()),
            })
        );
        assert!(initial_act_info(2_027, &[1150, 101, 4_000]).is_none());
        assert!(initial_act_info(2_027, &[1150, 102, 0]).is_none());
    }

    #[test]
    fn losing_hp_accumulates_the_same_fraction_of_remembrance_before_damage_publish() {
        crate::test_support::init_config();
        let mut managers = BattleManagers::seeded(&fight(0));
        let updates = hp_loss_state_update(&managers, &feature(&managers), 2_786)
            .expect("Remembrance state is valid");
        assert!(matches!(
            updates.as_slice(),
            [super::super::HpLossStateUpdate {
                command: BuffCommand::SetInternalState(BuffSetState {
                    act_info: Some(info), ..
                }),
                marker: BuffActInfoMarkerResult {
                    target_uid: 20,
                    buff_uid: 30,
                    act_id: 1150,
                    params,
                    ..
                },
                after_hp: false,
            }] if info[0].param == [2_071, 8_108] && params == &[2_071, 8_108]
        ));

        managers.execute_buff(updates[0].command.clone()).unwrap();
        assert_eq!(state(&managers, 20).unwrap().unwrap().current, 2_071);
    }

    #[test]
    fn losing_hp_caps_remembrance_at_its_initialized_maximum() {
        crate::test_support::init_config();
        let managers = BattleManagers::seeded(&fight(8_000));
        let updates = hp_loss_state_update(&managers, &feature(&managers), 2_786).unwrap();
        assert_eq!(updates[0].marker.params, [8_108, 8_108]);
    }

    #[test]
    fn damage_records_zero_consumption_for_stored_remembrance() {
        crate::test_support::init_config();
        let managers = BattleManagers::seeded(&fight(2_071));

        assert_eq!(
            damage_consumption(&managers, 20),
            Ok(Some(crate::engine::manager::hp::FakeHpConsumption {
                buff_uid: 30,
                consumed: 0,
            }))
        );
    }

    #[test]
    fn hp_execution_commits_and_publishes_remembrance_before_the_damage() {
        crate::test_support::init_config();
        let mut managers = BattleManagers::seeded(&fight(0));
        let execution = managers.execute_rule_hp(damage(20, 2_786)).unwrap();

        assert_eq!(state(&managers, 20).unwrap().unwrap().current, 2_071);
        assert_eq!(
            execution.changes.fake_hp_consumed,
            Some(crate::engine::manager::hp::FakeHpConsumption {
                buff_uid: 30,
                consumed: 0,
            })
        );
        let changes = RuleOutcome::Hp(Box::new(execution)).changes();
        assert!(matches!(
            changes.as_slice(),
            [
                BattleChange::BuffActInfoMarker(BuffActInfoMarkerResult {
                    act_id: 1150,
                    params,
                    ..
                }),
                BattleChange::Hp(_)
            ] if params == &[2_071, 8_108]
        ));
    }

    #[test]
    fn granting_remembrance_embeds_state_without_a_separate_initial_marker() {
        crate::test_support::init_config();
        let mut initial = fight(0);
        initial.attacker.as_mut().unwrap().entitys[1].buffs.clear();
        initial.attacker.as_mut().unwrap().entitys[0]
            .buffs
            .push(BuffInfo {
                uid: Some(31),
                buff_id: Some(104310124),
                from_uid: Some(10),
                layer: Some(1),
                ..Default::default()
            });
        let mut managers = BattleManagers::seeded(&initial);
        assert_eq!(managers.attribute.base(10, AttrId::Attack), 1_622);
        assert_eq!(managers.origin_attribute(10, AttrId::Attack), 2_027);
        let changes = managers
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: origin(),
                source_uid: 10,
                target_uid: 20,
                buff_id: 31450143,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap();
        let added = changes.change.added.expect("Remembrance should be granted");

        assert_eq!(added.buff.act_info[0].param, [0, 8_108]);
        assert!(added.pre_markers.is_empty());
    }

    #[test]
    fn duplicate_remembrance_state_rejects_hp_loss_before_damage() {
        crate::test_support::init_config();
        let mut initial = fight(0);
        let mut duplicate = initial.attacker.as_ref().unwrap().entitys[1].buffs[0].clone();
        duplicate.uid = Some(31);
        initial.attacker.as_mut().unwrap().entitys[1]
            .buffs
            .push(duplicate);
        let mut managers = BattleManagers::seeded(&initial);

        assert_eq!(
            managers.execute_hp(damage(20, 100)),
            Err(crate::engine::manager::hp::HpCommandError::InvalidHpLossState)
        );
        assert_eq!(managers.hp.current(20), 10_903);
    }

    #[test]
    fn invalid_later_remembrance_state_keeps_the_hp_batch_atomic() {
        crate::test_support::init_config();
        let mut initial = fight(0);
        initial
            .attacker
            .as_mut()
            .unwrap()
            .entitys
            .push(FightEntityInfo {
                uid: Some(30),
                team_type: Some(1),
                current_hp: Some(1_000),
                attr: Some(HeroAttribute {
                    hp: Some(1_000),
                    ..Default::default()
                }),
                buffs: vec![BuffInfo {
                    uid: Some(40),
                    buff_id: Some(31450143),
                    from_uid: Some(10),
                    layer: Some(1),
                    act_info: vec![BuffActInfo {
                        act_id: Some(1150),
                        param: vec![0],
                        str_param: Some(String::new()),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            });
        let mut managers = BattleManagers::seeded(&initial);

        assert_eq!(
            managers.execute_hp_batch(vec![damage(20, 100), damage(30, 100)]),
            Err(crate::engine::manager::hp::HpCommandError::InvalidHpLossState)
        );
        assert_eq!(managers.hp.current(20), 10_903);
        assert_eq!(managers.hp.current(30), 1_000);
        assert_eq!(state(&managers, 20).unwrap().unwrap().current, 0);
    }
}
