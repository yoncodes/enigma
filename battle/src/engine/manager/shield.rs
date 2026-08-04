use crate::engine::{
    manager::{
        BattleManagers,
        buff::{
            BuffChanges, BuffCommand, BuffCommandError, BuffGrant, BuffMarkerResult, BuffPlan,
            BuffPolicy, BuffRefreshDuration, BuffSetState,
        },
        hp::{HpChanges, HpCommand, HpCommandError, ShieldGrant, TeamSharedShieldGain},
    },
    skill::buff_act::registry::BuffActKind,
    skill::rule::CommandOrigin,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldScope {
    Entity,
    TeamShared,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldCarrierUid {
    Definition,
    Child,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShieldCommand {
    pub origin: CommandOrigin,
    pub source_uid: i64,
    pub target_uid: i64,
    pub buff_id: i32,
    pub amount_attr: crate::engine::entity::attr::AttrId,
    pub amount_rate: i32,
    pub multiplier_bonus: Option<(crate::engine::entity::attr::AttrId, i32)>,
    pub max_attr: crate::engine::entity::attr::AttrId,
    pub max_rate: i32,
    pub scope: ShieldScope,
    pub carrier_uid: ShieldCarrierUid,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShieldChanges {
    command: ShieldCommand,
    pub buff: Option<BuffChanges>,
    pub hp: Option<HpChanges>,
    pub team_shared: Option<TeamSharedShieldGain>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShieldCommandError {
    Buff(BuffCommandError),
    Hp(HpCommandError),
}

impl From<BuffCommandError> for ShieldCommandError {
    fn from(value: BuffCommandError) -> Self {
        Self::Buff(value)
    }
}

impl From<HpCommandError> for ShieldCommandError {
    fn from(value: HpCommandError) -> Self {
        Self::Hp(value)
    }
}

struct PlannedBuff {
    plan: BuffPlan,
    expose: bool,
}

struct ShieldPlan {
    command: ShieldCommand,
    buff: Option<PlannedBuff>,
    hp: Option<HpCommand>,
    team_shared: Option<TeamSharedShieldGain>,
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    command: ShieldCommand,
) -> Result<ShieldChanges, ShieldCommandError> {
    let plan = plan(managers, command)?;
    Ok(commit(managers, plan))
}

fn plan(
    managers: &BattleManagers,
    command: ShieldCommand,
) -> Result<ShieldPlan, ShieldCommandError> {
    let carrier_uid = managers
        .buff
        .buff_family_carrier_uid(command.target_uid, command.buff_id);
    let basis = managers.origin_attribute(command.source_uid, command.amount_attr);
    let multiplier_bonus = command
        .multiplier_bonus
        .map(|(attr, rate)| (managers.origin_attribute(command.source_uid, attr), rate));
    let amount = shield_amount(basis, command.amount_rate, multiplier_bonus);
    let max = managers
        .origin_attribute(command.source_uid, command.max_attr)
        .saturating_mul(command.max_rate)
        / 1000;

    match command.scope {
        ShieldScope::Entity => {
            let buff = match carrier_uid {
                Some(buff_uid) => {
                    let current_duration = managers
                        .buff
                        .snapshot(command.target_uid, buff_uid)
                        .and_then(|buff| buff.duration)
                        .ok_or(ShieldCommandError::Buff(
                            BuffCommandError::InvalidDurationChange,
                        ))?;
                    let configured_duration = BuffPolicy::try_for_buff_id(command.buff_id)
                        .map_err(BuffCommandError::InvalidPolicy)?
                        .lifetime
                        .duration;
                    (current_duration > 0 && configured_duration > 0)
                        .then(|| {
                            managers.plan_buff(BuffCommand::RefreshDuration(
                                BuffRefreshDuration {
                                    origin: command.origin,
                                    target_uid: command.target_uid,
                                    buff_uid,
                                    minimum_duration: configured_duration,
                                },
                            ))
                        })
                        .transpose()?
                }
                None => Some(plan_carrier(managers, command)?),
            };
            let accepted =
                carrier_uid.is_some() || buff.as_ref().and_then(BuffPlan::added_buff_uid).is_some();
            let hp = accepted.then_some(HpCommand::GrantShield(ShieldGrant {
                origin: command.origin,
                source_uid: command.source_uid,
                target_uid: command.target_uid,
                amount,
                max,
            }));
            if let Some(hp) = hp {
                managers.hp.validate_command(hp)?;
            }
            Ok(ShieldPlan {
                command,
                buff: buff.map(|plan| PlannedBuff { plan, expose: true }),
                hp,
                team_shared: None,
            })
        }
        ShieldScope::TeamShared => {
            let act_id = configured_act_id(command.buff_id, BuffActKind::TeamShareShield)
                .ok_or(ShieldCommandError::Buff(BuffCommandError::InvalidSetState))?;
            let max = max.max(0);
            let before = carrier_uid
                .and_then(|buff_uid| managers.buff.snapshot(command.target_uid, buff_uid))
                .and_then(|buff| {
                    buff.act_info
                        .iter()
                        .find(|info| info.act_id == Some(act_id))
                        .and_then(|info| info.param.first())
                        .copied()
                })
                .unwrap_or_default();
            let after = before.saturating_add(amount.max(0)).min(max);
            let (buff, buff_uid) = if let Some(buff_uid) = carrier_uid {
                let mut snapshot = managers
                    .buff
                    .snapshot(command.target_uid, buff_uid)
                    .ok_or(ShieldCommandError::Buff(BuffCommandError::InvalidSetState))?;
                upsert_act_info(&mut snapshot.act_info, act_id, after);
                sort_act_info(command.buff_id, &mut snapshot.act_info);
                (
                    Some(PlannedBuff {
                        plan: managers.plan_buff(BuffCommand::SetInternalState(BuffSetState {
                            origin: command.origin,
                            target_uid: command.target_uid,
                            buff_uid,
                            ex_info: None,
                            params: None,
                            act_info: Some(snapshot.act_info),
                        }))?,
                        expose: false,
                    }),
                    Some(buff_uid),
                )
            } else {
                let mut plan = plan_carrier(managers, command)?;
                let team_type = managers
                    .buff
                    .team_type(command.target_uid)
                    .ok_or(ShieldCommandError::Buff(BuffCommandError::InvalidSetState))?;
                let buff_uid = plan.initialize_added_act_value(
                    act_id,
                    after,
                    team_type,
                    managers.hp.current(command.target_uid),
                );
                (Some(PlannedBuff { plan, expose: true }), buff_uid)
            };
            let team_shared = buff_uid.map(|buff_uid| TeamSharedShieldGain {
                buff_uid,
                owner_uid: command.target_uid,
                buff_act_id: act_id,
                before,
                added: after - before,
                after,
                max,
            });
            Ok(ShieldPlan {
                command,
                buff,
                hp: None,
                team_shared,
            })
        }
    }
}

fn shield_amount(basis: i32, rate: i32, multiplier_bonus: Option<(i32, i32)>) -> i32 {
    let rate = i128::from(rate) * 1_000
        + multiplier_bonus.map_or(0, |(multiplier, bonus_rate)| {
            i128::from(multiplier) * i128::from(bonus_rate)
        });
    let amount = i128::from(basis) * rate / 1_000_000;
    amount.clamp(i128::from(i32::MIN), i128::from(i32::MAX)) as i32
}

fn plan_carrier(
    managers: &BattleManagers,
    command: ShieldCommand,
) -> Result<BuffPlan, ShieldCommandError> {
    let grant = BuffGrant {
        origin: command.origin,
        source_uid: command.source_uid,
        target_uid: command.target_uid,
        buff_id: command.buff_id,
        amount: None,
        occurrences: 1,
        child_uid_reservations: 0,
    };
    Ok(managers.plan_buff(match command.carrier_uid {
        ShieldCarrierUid::Definition => BuffCommand::Grant(grant),
        ShieldCarrierUid::Child => BuffCommand::GrantUsingChildUid(grant),
    })?)
}

fn commit(managers: &mut BattleManagers, plan: ShieldPlan) -> ShieldChanges {
    let mut buff = plan.buff.and_then(|planned| {
        let changes = managers.commit_buff(planned.plan);
        planned.expose.then_some(changes)
    });
    let hp = plan.hp.map(|command| {
        managers
            .hp
            .commit_validated_command_with_team_shared(command, None)
    });
    if let Some(gain) = plan.team_shared
        && let Some(added) = buff
            .as_mut()
            .and_then(|changes| changes.change.added.as_mut())
    {
        added.markers.insert(
            0,
            BuffMarkerResult {
                target_uid: gain.owner_uid,
                effect_type: sonettobuf::effect_type_enum::EffectType::None as i32,
                effect_num: gain.after,
                buff_act_id: 0,
            },
        );
    }
    ShieldChanges {
        command: plan.command,
        buff,
        hp,
        team_shared: plan.team_shared,
    }
}

fn configured_act_id(buff_id: i32, kind: BuffActKind) -> Option<i32> {
    crate::engine::manager::buff::BuffManager::configured_features(buff_id)
        .into_iter()
        .find_map(|feature| {
            let act_id = feature.act_id()?;
            crate::engine::skill::buff_act::registry::find(act_id, &feature.act_type)
                .filter(|definition| definition.kind == kind)
                .map(|_| act_id)
        })
}

fn upsert_act_info(act_info: &mut Vec<sonettobuf::BuffActInfo>, act_id: i32, value: i32) {
    if let Some(info) = act_info.iter_mut().find(|info| info.act_id == Some(act_id)) {
        info.param = vec![value];
        info.str_param = Some(String::new());
    } else {
        act_info.push(sonettobuf::BuffActInfo {
            act_id: Some(act_id),
            param: vec![value],
            str_param: Some(String::new()),
        });
    }
}

fn sort_act_info(buff_id: i32, act_info: &mut [sonettobuf::BuffActInfo]) {
    let order = crate::engine::manager::buff::BuffManager::configured_features(buff_id)
        .into_iter()
        .filter_map(|feature| feature.act_id())
        .collect::<Vec<_>>();
    act_info.sort_by_key(|info| {
        order
            .iter()
            .position(|act_id| Some(*act_id) == info.act_id)
            .unwrap_or(usize::MAX)
    });
}

#[cfg(test)]
mod tests {
    use sonettobuf::{Fight, FightEntityInfo, FightTeam, HeroAttribute, HeroExAttribute};

    use super::*;
    use crate::engine::skill::rule::{DefinitionKey, RuleDomain};

    fn command() -> ShieldCommand {
        ShieldCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60259, "SupplyShield2"),
            },
            source_uid: 1,
            target_uid: 1,
            buff_id: 31170002,
            amount_attr: crate::engine::entity::attr::AttrId::Attack,
            amount_rate: 1_500,
            multiplier_bonus: Some((crate::engine::entity::attr::AttrId::CriticalRate, 900)),
            max_attr: crate::engine::entity::attr::AttrId::Attack,
            max_rate: 6_500,
            scope: ShieldScope::Entity,
            carrier_uid: ShieldCarrierUid::Definition,
        }
    }

    #[test]
    fn shield_terms_are_rounded_after_they_are_combined() {
        assert_eq!(shield_amount(1_657, 1_500, Some((323, 400))), 2_699);
        assert_eq!(shield_amount(1_657, 2_250, Some((323, 600))), 4_049);
        assert_eq!(shield_amount(1_737, 1_500, Some((443, 400))), 2_913);
    }

    #[test]
    fn origin_attribute_includes_snapshotted_stateful_buff_values() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(2),
                        buff_id: Some(31340007),
                        from_uid: Some(1),
                        act_info: vec![sonettobuf::BuffActInfo {
                            act_id: Some(1053),
                            param: vec![18],
                            ..Default::default()
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        managers.attribute.override_ex(
            1,
            &HeroExAttribute {
                cri: Some(100),
                ..Default::default()
            },
        );

        assert_eq!(
            managers.origin_attribute(1, crate::engine::entity::attr::AttrId::CriticalRate),
            118
        );
    }

    #[test]
    fn shield_buff_is_attached_once_and_repeats_stack_to_the_cap() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        managers.attribute.override_ex(
            1,
            &HeroExAttribute {
                cri: Some(100),
                ..Default::default()
            },
        );

        let first = execute(&mut managers, command()).unwrap();
        let added = first
            .buff
            .as_ref()
            .and_then(|changes| changes.change.added.as_ref())
            .unwrap();
        assert_eq!(added.buff.uid, Some(2));
        assert_eq!(managers.hp.shield(1), 1_590);

        let second = execute(&mut managers, command()).unwrap();
        let refreshed = &second.buff.as_ref().unwrap().change.refreshed;
        assert_eq!(refreshed.len(), 1);
        assert_eq!(refreshed[0].after.uid, Some(2));
        assert_eq!(refreshed[0].after.duration, added.buff.duration);
        assert_eq!(second.hp.unwrap().shield_granted.unwrap().added, 1_590);
        assert_eq!(managers.hp.shield(1), 3_180);
    }

    #[test]
    fn assist_boss_hp_can_drive_its_configured_shield() {
        crate::test_support::init_config();
        let mut fight = Fight {
            attacker: Some(FightTeam {
                assist_boss: Some(FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(999_999),
                    attr: Some(HeroAttribute {
                        hp: Some(999_999),
                        attack: Some(7_001),
                        defense: Some(7_002),
                        mdefense: Some(7_003),
                        technic: Some(7_004),
                        ..Default::default()
                    }),
                    ex_point: Some(7),
                    ex_point_type: Some(3),
                    buffs: vec![sonettobuf::BuffInfo {
                        uid: Some(900),
                        buff_id: Some(999_999_999),
                        from_uid: Some(-1),
                        ..Default::default()
                    }],
                    team_type: Some(1),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        execute(
            &mut managers,
            ShieldCommand {
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: DefinitionKey::new(501, "Shield"),
                },
                source_uid: -1,
                target_uid: -1,
                buff_id: 4_700_101,
                amount_attr: crate::engine::entity::attr::AttrId::Hp,
                amount_rate: 600,
                multiplier_bonus: None,
                max_attr: crate::engine::entity::attr::AttrId::Hp,
                max_rate: 600,
                scope: ShieldScope::Entity,
                carrier_uid: ShieldCarrierUid::Definition,
            },
        )
        .unwrap();

        managers.sync_entities(&mut fight);

        assert_eq!(managers.hp.shield(-1), 599_999);
        let assist = fight.attacker.unwrap().assist_boss.unwrap();
        assert_eq!(assist.shield_value, Some(599_999));
        assert_eq!(assist.ex_point, Some(7));
        assert_eq!(assist.ex_point_type, Some(3));
        assert_eq!(
            assist.attr,
            Some(HeroAttribute {
                hp: Some(999_999),
                attack: Some(7_001),
                defense: Some(7_002),
                mdefense: Some(7_003),
                technic: Some(7_004),
                ..Default::default()
            })
        );
        assert!(assist.buffs.iter().any(|buff| buff.uid == Some(900)));
        assert_eq!(managers.buff.team_type(-1), Some(1));
        assert!(!managers.buff.alive_team_uids(1, &managers.hp).contains(&-1));
    }

    #[test]
    fn shield_variants_share_the_configured_type_carrier() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);

        let first = execute(&mut managers, command()).unwrap();
        let carrier_uid = first
            .buff
            .as_ref()
            .and_then(|changes| changes.change.added.as_ref())
            .and_then(|added| added.buff.uid)
            .unwrap();

        let mut stronger = command();
        stronger.buff_id = 31170009;
        stronger.amount_rate = 2_700;
        stronger.multiplier_bonus = None;
        let second = execute(&mut managers, stronger).unwrap();

        let refreshed = &second.buff.as_ref().unwrap().change.refreshed;
        assert_eq!(refreshed.len(), 1);
        assert_eq!(refreshed[0].after.uid, Some(carrier_uid));
        assert_eq!(refreshed[0].after.buff_id, Some(31170002));
        assert_eq!(second.hp.unwrap().shield_granted.unwrap().added, 2_700);
        assert_eq!(managers.hp.shield(1), 4_200);
        assert_eq!(
            managers
                .buff
                .active_for(1)
                .filter_map(|buff| buff.buff_id)
                .collect::<Vec<_>>(),
            vec![31170002]
        );
    }

    #[test]
    fn timed_shield_does_not_replace_or_expire_a_permanent_carrier() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let mut permanent = command();
        permanent.buff_id = 610161;

        let first = execute(&mut managers, permanent).unwrap();
        let carrier = first
            .buff
            .as_ref()
            .and_then(|changes| changes.change.added.as_ref())
            .unwrap();
        assert_eq!(carrier.buff.duration, Some(0));

        let timed = execute(&mut managers, command()).unwrap();
        assert!(timed.buff.is_none());
        let active = managers.buff.active_for(1).collect::<Vec<_>>();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].buff_id, Some(610161));
        assert_eq!(active[0].duration, Some(0));
    }

    #[test]
    fn shared_family_shield_carrier_uses_its_configured_uid_lane() {
        crate::test_support::init_config();
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        managers
            .execute_buff(BuffCommand::Grant(BuffGrant {
                origin: command().origin,
                source_uid: 1,
                target_uid: 1,
                buff_id: 5_230_012,
                amount: None,
                occurrences: 1,
                child_uid_reservations: 0,
            }))
            .unwrap();
        let mut shield = command();
        shield.buff_id = 222_001_232;
        shield.amount_attr = crate::engine::entity::attr::AttrId::Hp;
        shield.amount_rate = 300;
        shield.max_attr = crate::engine::entity::attr::AttrId::Hp;
        shield.max_rate = 300;
        shield.multiplier_bonus = None;

        let changes = execute(&mut managers, shield).unwrap();

        assert_eq!(
            changes.buff.unwrap().change.added.unwrap().buff.uid,
            Some(1004)
        );
    }

    #[test]
    fn child_carrier_commands_allocate_consecutive_uids_after_observed_state() {
        crate::test_support::init_config();
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: (1..=4)
                    .map(|uid| FightEntityInfo {
                        uid: Some(uid),
                        current_hp: Some(1_000),
                        attr: Some(HeroAttribute {
                            hp: Some(1_000),
                            attack: Some(1_000),
                            ..Default::default()
                        }),
                        buffs: (uid == 1)
                            .then(|| sonettobuf::BuffInfo {
                                buff_id: Some(434735),
                                uid: Some(1081),
                                from_uid: Some(1),
                                ..Default::default()
                            })
                            .into_iter()
                            .collect(),
                        ..Default::default()
                    })
                    .collect(),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);

        let actual = (1..=4)
            .map(|target_uid| {
                let mut shield = command();
                shield.target_uid = target_uid;
                shield.carrier_uid = ShieldCarrierUid::Child;
                execute(&mut managers, shield)
                    .unwrap()
                    .buff
                    .unwrap()
                    .change
                    .added
                    .unwrap()
                    .buff
                    .uid
                    .unwrap()
            })
            .collect::<Vec<_>>();

        assert_eq!(actual, vec![1082, 1083, 1084, 1085]);
    }

    #[test]
    fn team_shared_shield_stacks_on_its_buff_act_state() {
        crate::test_support::init_config();
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let command = ShieldCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60290, "SupplyTeamShareShield"),
            },
            source_uid: 1,
            target_uid: 1,
            buff_id: 31430144,
            amount_attr: crate::engine::entity::attr::AttrId::Attack,
            amount_rate: 2_800,
            multiplier_bonus: None,
            max_attr: crate::engine::entity::attr::AttrId::Attack,
            max_rate: 12_500,
            scope: ShieldScope::TeamShared,
            carrier_uid: ShieldCarrierUid::Definition,
        };

        let first = execute(&mut managers, command).unwrap();
        let added = first
            .buff
            .as_ref()
            .and_then(|changes| changes.change.added.as_ref())
            .unwrap();
        let buff_uid = added.buff.uid.unwrap();
        assert_eq!(
            added
                .buff
                .act_info
                .iter()
                .map(|info| (info.act_id.unwrap(), info.param.clone()))
                .collect::<Vec<_>>(),
            vec![(1125, vec![2_800]), (1126, vec![4])]
        );
        assert_eq!(added.pre_markers[0].act_id, 1126);
        assert_eq!(added.pre_markers[0].params, vec![4]);
        assert_eq!(
            added.markers[0],
            BuffMarkerResult {
                target_uid: 1,
                effect_type: sonettobuf::effect_type_enum::EffectType::None as i32,
                effect_num: 2_800,
                buff_act_id: 0,
            }
        );
        let second = execute(&mut managers, command).unwrap();
        assert!(second.buff.is_none());
        assert_eq!(second.team_shared.unwrap().after, 5_600);
        assert_eq!(
            managers
                .buff
                .snapshot(1, buff_uid)
                .unwrap()
                .act_info
                .iter()
                .find(|info| info.act_id == Some(1125))
                .unwrap()
                .param,
            vec![5_600]
        );

        managers
            .execute_buff(BuffCommand::Remove(
                crate::engine::manager::buff::BuffRemove {
                    origin: command.origin,
                    target_uid: 1,
                    selector: crate::engine::manager::buff::BuffRemoveSelector::ExactId(
                        command.buff_id,
                    ),
                },
            ))
            .unwrap();
        assert!(managers.buff.snapshot(1, buff_uid).is_none());
    }

    #[test]
    fn team_shared_shield_stacks_on_an_existing_shared_type_variant() {
        crate::test_support::init_config();
        let fight = Fight {
            version: Some(7),
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_948),
                        ..Default::default()
                    }),
                    buffs: vec![sonettobuf::BuffInfo {
                        buff_id: Some(31430144),
                        uid: Some(1073),
                        from_uid: Some(1),
                        act_info: vec![sonettobuf::BuffActInfo {
                            act_id: Some(1125),
                            param: vec![5_454],
                            str_param: Some(String::new()),
                        }],
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let command = ShieldCommand {
            origin: CommandOrigin {
                domain: RuleDomain::Behavior,
                key: DefinitionKey::new(60290, "SupplyTeamShareShield"),
            },
            source_uid: 1,
            target_uid: 1,
            buff_id: 31430121,
            amount_attr: crate::engine::entity::attr::AttrId::Attack,
            amount_rate: 2_800,
            multiplier_bonus: None,
            max_attr: crate::engine::entity::attr::AttrId::Attack,
            max_rate: 12_500,
            scope: ShieldScope::TeamShared,
            carrier_uid: ShieldCarrierUid::Definition,
        };

        let changes = execute(&mut managers, command).unwrap();

        assert!(changes.buff.is_none());
        assert_eq!(changes.team_shared.unwrap().after, 10_908);
        let carrier = managers.buff.snapshot(1, 1073).unwrap();
        assert_eq!(carrier.buff_id, Some(31430144));
        assert_eq!(
            carrier
                .act_info
                .iter()
                .find(|info| info.act_id == Some(1125))
                .unwrap()
                .param,
            vec![10_908]
        );
    }

    #[test]
    fn removing_the_last_shield_buff_clears_its_shield_value() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        execute(&mut managers, command()).unwrap();

        managers
            .execute_buff(BuffCommand::Remove(
                crate::engine::manager::buff::BuffRemove {
                    origin: command().origin,
                    target_uid: 1,
                    selector: crate::engine::manager::buff::BuffRemoveSelector::ExactId(
                        command().buff_id,
                    ),
                },
            ))
            .unwrap();

        assert_eq!(managers.hp.shield(1), 0);
    }

    #[test]
    fn failed_shield_planning_does_not_attach_a_carrier_or_advance_its_uid_lane() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(1),
                    current_hp: Some(1_000),
                    attr: Some(HeroAttribute {
                        hp: Some(1_000),
                        attack: Some(1_000),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);
        let mut invalid = command();
        invalid.amount_rate = 0;

        assert_eq!(
            execute(&mut managers, invalid),
            Err(ShieldCommandError::Hp(HpCommandError::InvalidCommand))
        );
        assert!(!managers.buff.has_buff_id(1, invalid.buff_id));
        assert_eq!(managers.hp.shield(1), 0);

        let valid = execute(&mut managers, command()).unwrap();
        assert_eq!(valid.buff.unwrap().change.added.unwrap().buff.uid, Some(2));
    }
}
