use crate::engine::{
    event::payload::{BattleEvent, ShellChangeEvent},
    manager::{
        BattleManagers,
        buff::{
            BuffChanges, BuffCommand, BuffCommandError, BuffConsume, BuffGrant, BuffSelector,
            DepletedBuff,
        },
    },
    skill::rule::CommandOrigin,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellChangeKind {
    Deployed,
    Retrieved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellCommand {
    Deploy {
        origin: CommandOrigin,
        source_uid: i64,
        target_uid: i64,
        stock_buff_id: i32,
        amount: i32,
    },
    Retrieve {
        origin: CommandOrigin,
        source_uid: i64,
        target_uid: i64,
        stock_buff_id: i32,
        amount: i32,
    },
    RetrieveAll {
        origin: CommandOrigin,
        source_uid: i64,
        stock_buff_id: i32,
    },
    AccumulateAndUseSkill {
        rule: crate::engine::skill::rule::ConfiguredRuleKey,
        source_uid: i64,
        target_uid: i64,
        threshold: i32,
        delta: i32,
        skill_id: i32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellChanges {
    pub buffs: Vec<BuffChanges>,
    pub events: Vec<ShellChangeEvent>,
    pub skills: Vec<crate::engine::skill::action::SkillInvocation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellError {
    InvalidCommand,
    MissingStock,
    MissingDeployedShell,
    Buff(BuffCommandError),
}

impl From<BuffCommandError> for ShellError {
    fn from(value: BuffCommandError) -> Self {
        Self::Buff(value)
    }
}

pub(crate) fn execute(
    managers: &mut BattleManagers,
    command: ShellCommand,
) -> Result<ShellChanges, ShellError> {
    match command {
        ShellCommand::Deploy {
            origin,
            source_uid,
            target_uid,
            stock_buff_id,
            amount,
        } => deploy(
            managers,
            origin,
            source_uid,
            target_uid,
            stock_buff_id,
            amount,
        ),
        ShellCommand::Retrieve {
            origin,
            source_uid,
            target_uid,
            stock_buff_id,
            amount,
        } => retrieve(
            managers,
            origin,
            source_uid,
            target_uid,
            stock_buff_id,
            amount,
        ),
        ShellCommand::RetrieveAll {
            origin,
            source_uid,
            stock_buff_id,
        } => retrieve_all(managers, origin, source_uid, stock_buff_id),
        ShellCommand::AccumulateAndUseSkill {
            rule,
            source_uid,
            target_uid,
            threshold,
            delta,
            skill_id,
        } => accumulate_and_use_skill(
            managers, rule, source_uid, target_uid, threshold, delta, skill_id,
        ),
    }
}

fn deploy(
    managers: &mut BattleManagers,
    origin: CommandOrigin,
    source_uid: i64,
    target_uid: i64,
    stock_buff_id: i32,
    requested: i32,
) -> Result<ShellChanges, ShellError> {
    let deployed_buff_id = runtime_deployed_buff_id(managers, stock_buff_id)?;
    let available = managers
        .buff
        .buff_id_amount(source_uid, stock_buff_id)
        .max(0);
    let amount = if requested < 0 {
        available
    } else {
        requested.min(available)
    };
    if source_uid == 0 || target_uid == 0 || requested == 0 {
        return Err(ShellError::MissingStock);
    }
    // Reactions planned together can each request a deploy; once the stock is spent the rest deploy nothing.
    if amount <= 0 {
        return Ok(ShellChanges {
            buffs: Vec::new(),
            events: Vec::new(),
            skills: Vec::new(),
        });
    }

    let buffs = vec![
        managers.execute_buff(BuffCommand::Consume(BuffConsume {
            origin,
            target_uid: source_uid,
            selector: BuffSelector::IdOrType(stock_buff_id),
            amount,
            depleted: DepletedBuff::Remove,
        }))?,
        managers.execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid,
            target_uid,
            buff_id: deployed_buff_id,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        }))?,
    ];
    Ok(ShellChanges {
        buffs,
        events: vec![ShellChangeEvent {
            kind: ShellChangeKind::Deployed,
            source_uid,
            target_uid,
            stock_buff_id,
            deployed_buff_id,
            amount,
            transaction_amount: amount,
            settles_transaction: true,
        }],
        skills: Vec::new(),
    })
}

fn retrieve_all(
    managers: &mut BattleManagers,
    origin: CommandOrigin,
    source_uid: i64,
    stock_buff_id: i32,
) -> Result<ShellChanges, ShellError> {
    let deployed_buff_id = runtime_deployed_buff_id(managers, stock_buff_id)?;
    let mut deployed = managers
        .buff
        .active_features(&managers.hp)
        .into_iter()
        .filter(|feature| feature.buff_id == deployed_buff_id && feature.amount > 0)
        .filter(|feature| {
            crate::engine::skill::buff_act::is_kind(
                feature,
                crate::engine::skill::buff_act::registry::BuffActKind::ShellProcess,
            )
        })
        .map(|feature| (feature.owner_uid, feature.amount))
        .collect::<Vec<_>>();
    deployed.sort_unstable_by_key(|(target_uid, _)| *target_uid);
    if source_uid == 0 {
        return Err(ShellError::MissingDeployedShell);
    }

    let mut changes = ShellChanges {
        buffs: Vec::with_capacity(deployed.len() * 2),
        events: Vec::with_capacity(deployed.len()),
        skills: Vec::new(),
    };
    for (target_uid, amount) in deployed {
        let retrieved = retrieve(
            managers,
            origin,
            source_uid,
            target_uid,
            stock_buff_id,
            amount,
        )?;
        changes.buffs.extend(retrieved.buffs);
        changes.events.extend(retrieved.events);
    }
    let transaction_amount = changes.events.iter().map(|event| event.amount).sum();
    let last = changes.events.len().saturating_sub(1);
    for (index, event) in changes.events.iter_mut().enumerate() {
        event.transaction_amount = transaction_amount;
        event.settles_transaction = index == last;
    }
    Ok(changes)
}

fn retrieve(
    managers: &mut BattleManagers,
    origin: CommandOrigin,
    source_uid: i64,
    target_uid: i64,
    stock_buff_id: i32,
    requested: i32,
) -> Result<ShellChanges, ShellError> {
    let deployed_buff_id = runtime_deployed_buff_id(managers, stock_buff_id)?;
    let available = managers
        .buff
        .buff_id_amount(target_uid, deployed_buff_id)
        .max(0);
    let amount = if requested < 0 {
        available
    } else {
        requested.min(available)
    };
    if source_uid == 0 || target_uid == 0 || amount <= 0 {
        return Err(ShellError::MissingDeployedShell);
    }
    let buffs = vec![
        managers.execute_buff(BuffCommand::ConsumeCoalesced(BuffConsume {
            origin,
            target_uid,
            selector: BuffSelector::IdOrType(deployed_buff_id),
            amount,
            depleted: DepletedBuff::Remove,
        }))?,
        managers.execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid,
            target_uid: source_uid,
            buff_id: stock_buff_id,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        }))?,
    ];
    Ok(ShellChanges {
        buffs,
        events: vec![ShellChangeEvent {
            kind: ShellChangeKind::Retrieved,
            source_uid,
            target_uid,
            stock_buff_id,
            deployed_buff_id,
            amount,
            transaction_amount: amount,
            settles_transaction: true,
        }],
        skills: Vec::new(),
    })
}

fn runtime_deployed_buff_id(
    managers: &BattleManagers,
    stock_buff_id: i32,
) -> Result<i32, ShellError> {
    let catalog = managers
        .buff
        .try_catalog()
        .or_else(crate::catalog::BattleCatalog::try_global)
        .ok_or(ShellError::InvalidCommand)?;
    crate::engine::skill::buff_act::shell::resolve_deployed_buff_id(catalog, stock_buff_id)
        .ok_or(ShellError::InvalidCommand)
}

fn accumulate_and_use_skill(
    managers: &mut BattleManagers,
    rule: crate::engine::skill::rule::ConfiguredRuleKey,
    source_uid: i64,
    target_uid: i64,
    threshold: i32,
    delta: i32,
    skill_id: i32,
) -> Result<ShellChanges, ShellError> {
    if source_uid == 0 || target_uid == 0 || threshold <= 0 || delta <= 0 || skill_id <= 0 {
        return Err(ShellError::InvalidCommand);
    }
    // Changes made while the triggered cast waits do not count toward the next one.
    let triggered =
        managers.advance_configured_rule_progress_until_cast(source_uid, rule, threshold, delta);
    let skills = triggered
        .then(|| {
            let mut invocation: crate::engine::skill::action::SkillInvocation =
                crate::engine::skill::action::SkillRequest {
                    source_uid,
                    skill_id,
                }
                .into();
            invocation.target = crate::engine::skill::action::SkillTarget::Explicit(target_uid);
            // Unmendable Cracks follows the triggering action's ally-action reactions.
            invocation.start = crate::engine::skill::action::SkillStart::AfterCurrentAction;
            invocation.release_progress = Some(rule);
            invocation
        })
        .into_iter()
        .collect();
    Ok(ShellChanges {
        buffs: Vec::new(),
        events: Vec::new(),
        skills,
    })
}

pub fn events(changes: &ShellChanges) -> impl Iterator<Item = BattleEvent> + '_ {
    changes
        .events
        .iter()
        .copied()
        .map(BattleEvent::ShellChanged)
}

#[cfg(test)]
mod tests {
    use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam};

    use super::*;
    use crate::engine::skill::rule::{ConfiguredRuleKey, DefinitionKey, RuleDomain};

    const ORIGIN: CommandOrigin = CommandOrigin {
        domain: RuleDomain::Behavior,
        key: DefinitionKey::new(60135, "ShellUseSkill"),
    };
    const RULE: ConfiguredRuleKey = ConfiguredRuleKey::new(31090114, 1, ORIGIN.key);

    #[test]
    fn changes_while_the_triggered_cast_waits_do_not_count() {
        let mut managers = BattleManagers::default();
        let casts_after = |managers: &mut BattleManagers, rule, changes: i32| {
            (0..changes)
                .map(|_| {
                    execute(
                        managers,
                        ShellCommand::AccumulateAndUseSkill {
                            rule,
                            source_uid: 10,
                            target_uid: -1,
                            threshold: 7,
                            delta: 1,
                            skill_id: 31090114,
                        },
                    )
                    .unwrap()
                    .skills
                    .len()
                })
                .sum::<usize>()
        };

        // "After accumulating a total of 7 deployments and or retrievals, triggers" one cast.
        assert_eq!(casts_after(&mut managers, RULE, 7), 1);
        assert_eq!(casts_after(&mut managers, RULE, 7), 0);
        managers.release_held_rule_progress(10, RULE);
        assert_eq!(casts_after(&mut managers, RULE, 6), 0);
        assert_eq!(casts_after(&mut managers, RULE, 1), 1);
    }

    #[test]
    fn releasing_a_cast_resumes_only_its_exact_rule() {
        let mut managers = BattleManagers::default();
        let other = ConfiguredRuleKey::new(RULE.effect_id, 2, RULE.definition);
        let trigger = |managers: &mut BattleManagers, rule| {
            (0..7).fold(0, |casts, _| {
                casts
                    + execute(
                        managers,
                        ShellCommand::AccumulateAndUseSkill {
                            rule,
                            source_uid: 10,
                            target_uid: -1,
                            threshold: 7,
                            delta: 1,
                            skill_id: 31090114,
                        },
                    )
                    .unwrap()
                    .skills
                    .len()
            })
        };

        assert_eq!(trigger(&mut managers, RULE), 1);
        assert_eq!(trigger(&mut managers, other), 1);
        managers.release_held_rule_progress(10, RULE);

        assert_eq!(trigger(&mut managers, RULE), 1);
        assert_eq!(trigger(&mut managers, other), 0);
    }

    #[test]
    fn deploy_after_the_stock_is_spent_deploys_nothing() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    team_type: Some(1),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    team_type: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);

        let changes = execute(
            &mut managers,
            ShellCommand::Deploy {
                origin: ORIGIN,
                source_uid: 10,
                target_uid: -1,
                stock_buff_id: 31090117,
                amount: 1,
            },
        )
        .unwrap();

        assert!(changes.buffs.is_empty() && changes.events.is_empty());
        assert_eq!(managers.buff.buff_id_amount(-1, 31090118), 0);
    }

    #[test]
    fn negative_deploy_amount_moves_all_stock_and_removes_the_spent_stock() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    team_type: Some(1),
                    buffs: vec![BuffInfo {
                        uid: Some(52),
                        buff_id: Some(31090117),
                        layer: Some(8),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(-1),
                    current_hp: Some(100),
                    team_type: Some(2),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);

        let changes = execute(
            &mut managers,
            ShellCommand::Deploy {
                origin: ORIGIN,
                source_uid: 10,
                target_uid: -1,
                stock_buff_id: 31090117,
                amount: -1,
            },
        )
        .unwrap();

        assert_eq!(managers.buff.buff_id_amount(10, 31090117), 0);
        assert!(!managers.buff.has_buff_id(10, 31090117));
        assert_eq!(managers.buff.buff_id_amount(-1, 31090118), 8);
        assert_eq!(changes.events[0].amount, 8);
        // With the stock gone, the caster's shells are still found through the deployed ones.
        assert_eq!(
            crate::engine::skill::buff_act::shell::caster_shell_spec(&managers, 10)
                .map(|spec| spec.stock_buff_id),
            Some(31090117)
        );
    }

    #[test]
    fn retrieve_all_returns_every_deployed_stack_to_the_configured_stock() {
        crate::test_support::init_config();
        let entity = |uid, team_type, buff_id, buff_uid, layer| FightEntityInfo {
            uid: Some(uid),
            current_hp: Some(100),
            team_type: Some(team_type),
            buffs: vec![BuffInfo {
                uid: Some(buff_uid),
                buff_id: Some(buff_id),
                layer: Some(layer),
                ..Default::default()
            }],
            ..Default::default()
        };
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![entity(10, 1, 31090111, 52, 8)],
                ..Default::default()
            }),
            defender: Some(FightTeam {
                entitys: vec![
                    entity(-1, 2, 31090112, 100_001, 3),
                    entity(-2, 2, 31090112, 100_002, 2),
                ],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);

        let changes = execute(
            &mut managers,
            ShellCommand::RetrieveAll {
                origin: ORIGIN,
                source_uid: 10,
                stock_buff_id: 31090111,
            },
        )
        .unwrap();

        assert_eq!(managers.buff.buff_id_amount(-1, 31090112), 0);
        assert_eq!(managers.buff.buff_id_amount(-2, 31090112), 0);
        assert_eq!(managers.buff.buff_id_amount(10, 31090111), 13);
        assert_eq!(changes.buffs.len(), 4);
        assert_eq!(changes.events.len(), 2);
        assert_eq!(changes.events[0].kind, ShellChangeKind::Retrieved);
        assert_eq!(changes.events[0].target_uid, -2);
        assert_eq!(changes.events[0].amount, 2);
        assert_eq!(changes.events[1].target_uid, -1);
        assert_eq!(changes.events[1].amount, 3);
    }

    #[test]
    fn retrieve_all_without_deployed_shells_is_a_no_op() {
        crate::test_support::init_config();
        let fight = Fight {
            attacker: Some(FightTeam {
                entitys: vec![FightEntityInfo {
                    uid: Some(10),
                    current_hp: Some(100),
                    buffs: vec![BuffInfo {
                        uid: Some(52),
                        buff_id: Some(31090111),
                        layer: Some(8),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut managers = BattleManagers::seeded(&fight);

        let changes = execute(
            &mut managers,
            ShellCommand::RetrieveAll {
                origin: ORIGIN,
                source_uid: 10,
                stock_buff_id: 31090111,
            },
        )
        .unwrap();

        assert_eq!(managers.buff.buff_id_amount(10, 31090111), 8);
        assert!(changes.buffs.is_empty());
        assert!(changes.events.is_empty());
    }

    #[test]
    fn configured_progress_emits_one_skill_per_completed_threshold() {
        let mut managers = BattleManagers::default();
        let command = |delta| ShellCommand::AccumulateAndUseSkill {
            rule: RULE,
            source_uid: 10,
            target_uid: -1,
            threshold: 5,
            delta,
            skill_id: 31090174,
        };

        assert!(
            execute(&mut managers, command(3))
                .unwrap()
                .skills
                .is_empty()
        );
        let changes = execute(&mut managers, command(4)).unwrap();

        assert_eq!(changes.skills.len(), 1);
        assert_eq!(changes.skills[0].plan.skill_id, 31090174);
        assert_eq!(
            changes.skills[0].target,
            crate::engine::skill::action::SkillTarget::Explicit(-1)
        );
    }
}
