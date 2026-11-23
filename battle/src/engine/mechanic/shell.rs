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
            depleted: DepletedBuff::RemoveWithZeroLayer,
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
            depleted: DepletedBuff::RemoveWithZeroLayer,
        }))?,
        managers.execute_buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: target_uid,
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
            invocation.extra_skill_kind =
                Some(crate::engine::skill::condition::extra::ExtraSkillKind::ExtraAction);
            invocation.target_observed_extra_action = Some(false);
            invocation.mode = crate::engine::skill::action::SkillExecutionMode::Active;
            // Unmendable Cracks belongs to the action that completed the configured threshold.
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
mod tests;
