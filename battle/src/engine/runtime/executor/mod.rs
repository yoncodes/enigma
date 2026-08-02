use crate::engine::{
    event::bus::EventBus,
    manager::{
        BattleManagers, HpExecution,
        buff::{BuffChanges, BuffCommandError},
        card::{CardChanges, CardCommandError},
        conduit::{ConduitChange, ConduitError},
        emitter::EmitterChange,
        entity::{EntityChanges, EntityCommandError},
        eureka::{EurekaChanges, EurekaCommandError},
        ex_point::{ExPointChanges, ExPointCommandError},
        field::{FieldChange, FieldCommandError},
        gauge::{GaugeChange, GaugeCommandError},
        hp::HpChanges,
        shield::{ShieldChanges, ShieldCommandError},
        summon::{SummonChanges, SummonCommandError},
        upgrade::{UpgradeChange, UpgradeCommandError},
    },
    mechanic::{
        buff_precast::{BuffPrecastChanges, BuffPrecastError},
        field_transfer::{FieldTransferChanges, FieldTransferError},
        shell::{ShellChanges, ShellError},
    },
    runtime::change::BattleChange,
    skill::{
        behavior::registry::OutputOwner,
        buff_act::raspberry::{CapacityError, CapacityResult},
        rule::output::{BattleCommand, RuleOp},
    },
};

pub(crate) enum RuleOutcome {
    PublishedEvent,
    SkillLifecycle(crate::engine::skill::action::SkillLifecycle),
    SkillActionStarted {
        lifecycle: crate::engine::skill::action::SkillLifecycle,
        cost: ExPointChanges,
    },
    Buff(Box<BuffChanges>),
    BuffBatch(Vec<BuffChanges>),
    BuffFeatureMarker(crate::engine::manager::buff::BuffMarkerResult),
    EffectMarker(crate::engine::skill::rule::output::EffectMarker),
    SceneChange {
        scene_id: i32,
    },
    BuffActTrigger(crate::engine::manager::buff::BuffActTriggerResult),
    BuffActInfoMarker(crate::engine::manager::buff::BuffActInfoMarkerResult),
    StateChanged,
    NuoDiKaHit(crate::engine::mechanic::nuo_di_ka::NuoDiKaHit),
    Hp(Box<HpExecution>),
    HpBatch(Vec<HpExecution>),
    Injury(crate::engine::manager::injury::InjuryChange),
    Revive(Box<crate::engine::manager::revive::ReviveChanges>),
    Shield(Box<ShieldChanges>),
    ExPoint(ExPointChanges),
    Eureka(EurekaChanges),
    Gauge(GaugeChange),
    BloodtitheSpend(Box<crate::engine::mechanic::bloodtithe::spend::SpendChanges>),
    NuoDiKa(crate::engine::mechanic::nuo_di_ka::NuoDiKaChange),
    Emitter(EmitterChange),
    Entity(Box<EntityChanges>),
    Card(Box<CardChanges>),
    BuffPrecast(Box<BuffPrecastChanges>),
    Conduit(ConduitChange),
    Field(FieldChange),
    FieldTransfer(Box<FieldTransferChanges>),
    Shell(Box<ShellChanges>),
    RaspberryCapacity(Box<CapacityResult>),
    Summon(SummonChanges),
    Upgrade(UpgradeChange),
    ToughnessRecovered(crate::engine::manager::toughness::ToughnessRecovery),
    ThresholdSkills(Vec<crate::engine::skill::action::SkillInvocation>),
    ActiveSkillTargetsModified(i32),
}

impl RuleOutcome {
    pub(crate) fn followups(&self) -> Vec<RuleOp> {
        match self {
            Self::Shell(changes) => changes.skills.iter().cloned().map(RuleOp::Skill).collect(),
            Self::ThresholdSkills(skills) => skills.iter().cloned().map(RuleOp::Skill).collect(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn owned_changes(&self) -> Vec<(OutputOwner, BattleChange)> {
        match self {
            Self::BloodtitheSpend(changes) => vec![
                (OutputOwner::Parent, BattleChange::Gauge(changes.gauge)),
                (
                    OutputOwner::Skill,
                    BattleChange::Buff(Box::new(changes.buff.clone())),
                ),
            ],
            _ => Vec::new(),
        }
    }

    pub(crate) fn applied_damage(&self) -> i32 {
        match self {
            Self::Hp(execution) => execution.changes.applied_damage(),
            Self::HpBatch(changes) => changes
                .iter()
                .map(|execution| execution.changes.applied_damage())
                .sum(),
            _ => 0,
        }
    }

    pub(crate) fn death_count(&self) -> i32 {
        match self {
            Self::Hp(execution) => i32::from(execution.changes.caused_death()),
            Self::HpBatch(changes) => changes
                .iter()
                .filter(|execution| execution.changes.caused_death())
                .count() as i32,
            _ => 0,
        }
    }

    pub(crate) fn guard_break_count(&self) -> i32 {
        let broke =
            |change: &HpChanges| i32::from(change.toughness.is_some_and(|change| change.broke));
        match self {
            Self::Hp(execution) => broke(&execution.changes),
            Self::HpBatch(changes) => changes
                .iter()
                .map(|execution| broke(&execution.changes))
                .sum(),
            _ => 0,
        }
    }

    pub(crate) fn take_deaths(&mut self) -> Vec<crate::engine::manager::hp::DeathTransition> {
        match self {
            Self::Hp(execution) => execution.changes.death.take().into_iter().collect(),
            Self::HpBatch(changes) => changes
                .iter_mut()
                .filter_map(|execution| execution.changes.death.take())
                .collect(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn injured_targets(&self) -> Vec<i64> {
        let injured =
            |change: &HpChanges| change.hp.filter(|hp| hp.delta < 0).map(|hp| hp.target_uid);
        match self {
            Self::Hp(execution) => injured(&execution.changes).into_iter().collect(),
            Self::HpBatch(changes) => changes
                .iter()
                .filter_map(|execution| injured(&execution.changes))
                .collect(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn changes(&self) -> Vec<BattleChange> {
        match self {
            Self::PublishedEvent => Vec::new(),
            Self::SkillLifecycle(change) => vec![BattleChange::SkillLifecycle(change.clone())],
            Self::SkillActionStarted { lifecycle, cost } => vec![
                BattleChange::SkillLifecycle(lifecycle.clone()),
                BattleChange::ExPoint(*cost),
            ],
            Self::Buff(change) => vec![BattleChange::Buff(change.clone())],
            Self::BuffBatch(changes) => changes
                .iter()
                .cloned()
                .map(|change| BattleChange::Buff(Box::new(change)))
                .collect(),
            Self::BuffFeatureMarker(change) => vec![BattleChange::BuffFeatureMarker(*change)],
            Self::EffectMarker(change) => vec![BattleChange::EffectMarker(change.clone())],
            Self::SceneChange { scene_id } => vec![BattleChange::SceneChange {
                scene_id: *scene_id,
            }],
            Self::BuffActTrigger(change) => vec![BattleChange::BuffActTrigger(*change)],
            Self::BuffActInfoMarker(change) => {
                vec![BattleChange::BuffActInfoMarker(change.clone())]
            }
            Self::StateChanged => Vec::new(),
            Self::NuoDiKaHit(hit) => vec![BattleChange::NuoDiKaHit(*hit)],
            Self::Hp(execution) => {
                std::iter::once(BattleChange::Hp(Box::new(execution.changes.clone())))
                    .chain(execution.indicator.clone().map(BattleChange::EffectMarker))
                    .collect()
            }
            Self::HpBatch(changes) => changes
                .iter()
                .flat_map(|execution| {
                    std::iter::once(BattleChange::Hp(Box::new(execution.changes.clone())))
                        .chain(execution.indicator.clone().map(BattleChange::EffectMarker))
                })
                .collect(),
            Self::Injury(change) => vec![BattleChange::Injury(change.clone())],
            Self::Revive(changes) => changes
                .hp
                .iter()
                .map(|change| BattleChange::Hp(change.clone()))
                .chain(
                    changes
                        .buffs
                        .iter()
                        .cloned()
                        .map(|change| BattleChange::Buff(Box::new(change))),
                )
                .collect(),
            Self::Shield(changes) => vec![BattleChange::Shield(changes.clone())],
            Self::ExPoint(change) => vec![BattleChange::ExPoint(*change)],
            Self::Eureka(change) => vec![BattleChange::Eureka(change.clone())],
            Self::Gauge(change) => vec![BattleChange::Gauge(*change)],
            Self::BloodtitheSpend(_) => Vec::new(),
            Self::NuoDiKa(change) => vec![BattleChange::NuoDiKa(*change)],
            Self::Emitter(change) => vec![BattleChange::Emitter(*change)],
            Self::Entity(change) => vec![BattleChange::Entity(change.clone())],
            Self::Card(change) => vec![BattleChange::Card(change.clone())],
            Self::BuffPrecast(changes) => vec![
                BattleChange::Buff(Box::new(changes.buff.clone())),
                BattleChange::Card(Box::new(changes.card.clone())),
            ],
            Self::Conduit(change) => vec![BattleChange::Conduit(change.clone())],
            Self::Field(change) => vec![BattleChange::Field(*change)],
            Self::FieldTransfer(changes) => changes
                .buffs
                .iter()
                .cloned()
                .map(|change| BattleChange::Buff(Box::new(change)))
                .chain(std::iter::once(BattleChange::Field(changes.field)))
                .collect(),
            Self::Shell(changes) => changes
                .buffs
                .iter()
                .cloned()
                .map(|change| BattleChange::Buff(Box::new(change)))
                .collect(),
            Self::RaspberryCapacity(result) => {
                let ex_point = match result.as_ref() {
                    CapacityResult::Applied(changes) => changes.ex_point,
                    CapacityResult::AtCap(_) => None,
                };
                ex_point
                    .map(BattleChange::ExPoint)
                    .into_iter()
                    .chain(std::iter::once(BattleChange::RaspberryCapacity(
                        result.clone(),
                    )))
                    .collect()
            }
            Self::Summon(change) => vec![BattleChange::Summon(*change)],
            Self::Upgrade(change) => vec![BattleChange::Upgrade(change.clone())],
            Self::ToughnessRecovered(change) => vec![BattleChange::ToughnessRecovered(*change)],
            Self::ThresholdSkills(_) => Vec::new(),
            Self::ActiveSkillTargetsModified(_) => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuleExecutionError {
    Buff(BuffCommandError),
    Hp(crate::engine::manager::hp::HpCommandError),
    Shield(ShieldCommandError),
    ExPoint(ExPointCommandError),
    Eureka(EurekaCommandError),
    Entity(EntityCommandError),
    Gauge(GaugeCommandError),
    BloodtitheSpend(crate::engine::mechanic::bloodtithe::spend::SpendError),
    NuoDiKa(crate::engine::mechanic::nuo_di_ka::NuoDiKaError),
    Card(CardCommandError),
    BuffPrecast(BuffPrecastError),
    Conduit(ConduitError),
    Field(FieldCommandError),
    FieldTransfer(FieldTransferError),
    Shell(ShellError),
    RaspberryCapacity(CapacityError),
    Summon(SummonCommandError),
    Upgrade(UpgradeCommandError),
    Revive(crate::engine::manager::revive::ReviveError),
    UnexpectedSkill,
}

impl From<BuffCommandError> for RuleExecutionError {
    fn from(value: BuffCommandError) -> Self {
        Self::Buff(value)
    }
}

impl From<crate::engine::manager::hp::HpCommandError> for RuleExecutionError {
    fn from(value: crate::engine::manager::hp::HpCommandError) -> Self {
        Self::Hp(value)
    }
}

impl From<ShieldCommandError> for RuleExecutionError {
    fn from(value: ShieldCommandError) -> Self {
        Self::Shield(value)
    }
}

impl From<ExPointCommandError> for RuleExecutionError {
    fn from(value: ExPointCommandError) -> Self {
        Self::ExPoint(value)
    }
}

impl From<EurekaCommandError> for RuleExecutionError {
    fn from(value: EurekaCommandError) -> Self {
        Self::Eureka(value)
    }
}

impl From<EntityCommandError> for RuleExecutionError {
    fn from(value: EntityCommandError) -> Self {
        Self::Entity(value)
    }
}

impl From<GaugeCommandError> for RuleExecutionError {
    fn from(value: GaugeCommandError) -> Self {
        Self::Gauge(value)
    }
}

impl From<crate::engine::mechanic::bloodtithe::spend::SpendError> for RuleExecutionError {
    fn from(value: crate::engine::mechanic::bloodtithe::spend::SpendError) -> Self {
        Self::BloodtitheSpend(value)
    }
}

impl From<CardCommandError> for RuleExecutionError {
    fn from(value: CardCommandError) -> Self {
        Self::Card(value)
    }
}

impl From<BuffPrecastError> for RuleExecutionError {
    fn from(value: BuffPrecastError) -> Self {
        Self::BuffPrecast(value)
    }
}

impl From<ConduitError> for RuleExecutionError {
    fn from(value: ConduitError) -> Self {
        Self::Conduit(value)
    }
}

impl From<FieldCommandError> for RuleExecutionError {
    fn from(value: FieldCommandError) -> Self {
        Self::Field(value)
    }
}

impl From<FieldTransferError> for RuleExecutionError {
    fn from(value: FieldTransferError) -> Self {
        Self::FieldTransfer(value)
    }
}

impl From<ShellError> for RuleExecutionError {
    fn from(value: ShellError) -> Self {
        Self::Shell(value)
    }
}

impl From<CapacityError> for RuleExecutionError {
    fn from(value: CapacityError) -> Self {
        Self::RaspberryCapacity(value)
    }
}

impl From<SummonCommandError> for RuleExecutionError {
    fn from(value: SummonCommandError) -> Self {
        Self::Summon(value)
    }
}

impl From<UpgradeCommandError> for RuleExecutionError {
    fn from(value: UpgradeCommandError) -> Self {
        Self::Upgrade(value)
    }
}

impl From<crate::engine::manager::revive::ReviveError> for RuleExecutionError {
    fn from(value: crate::engine::manager::revive::ReviveError) -> Self {
        Self::Revive(value)
    }
}

pub(crate) fn execute_rule_op(
    managers: &mut BattleManagers,
    events: &mut EventBus,
    output: RuleOp,
) -> Result<RuleOutcome, RuleExecutionError> {
    match output {
        RuleOp::Publish(event) => {
            events.push(event);
            Ok(RuleOutcome::PublishedEvent)
        }
        RuleOp::Command(BattleCommand::Buff(command)) => {
            let changes = managers.execute_buff(command)?;
            for event in changes.events() {
                events.push(event);
            }
            Ok(RuleOutcome::Buff(Box::new(changes)))
        }
        RuleOp::Command(BattleCommand::BuffBatch(commands)) => {
            managers.buff.begin_transaction();
            let batch_result = (|| {
                let mut batch = Vec::with_capacity(commands.len());
                for command in commands {
                    batch.push(managers.execute_buff(command)?);
                }
                Ok::<_, RuleExecutionError>(batch)
            })();
            managers.buff.end_transaction();
            let batch = batch_result?;
            for changes in &batch {
                for event in changes.events() {
                    events.push(event);
                }
            }
            Ok(RuleOutcome::BuffBatch(batch))
        }
        RuleOp::Command(BattleCommand::Hp(command)) => {
            let command = crate::engine::skill::buff_act::fixed_hurt::resolve_command(
                &managers.buff,
                &managers.hp,
                command,
            );
            let execution = managers.execute_rule_hp(command)?;
            for event in execution.changes.events() {
                events.push(event);
            }
            Ok(RuleOutcome::Hp(Box::new(execution)))
        }
        RuleOp::Command(BattleCommand::HpBatch(commands)) => {
            let commands = commands
                .into_iter()
                .map(|command| {
                    crate::engine::skill::buff_act::fixed_hurt::resolve_command(
                        &managers.buff,
                        &managers.hp,
                        command,
                    )
                })
                .collect();
            let batch = managers.execute_rule_hp_batch(commands)?;
            for execution in &batch {
                for event in execution.changes.events() {
                    events.push(event);
                }
            }
            Ok(RuleOutcome::HpBatch(batch))
        }
        RuleOp::NuoDiKaHit(hit) => Ok(RuleOutcome::NuoDiKaHit(hit)),
        RuleOp::Command(BattleCommand::Injury(command)) => {
            Ok(RuleOutcome::Injury(managers.injury.execute(command)))
        }
        RuleOp::Command(BattleCommand::Revive(command)) => {
            let changes = crate::engine::manager::revive::execute(managers, command)?;
            if let Some(hp) = &changes.hp {
                for event in hp.events() {
                    events.push(event);
                }
            }
            for buff in &changes.buffs {
                for event in buff.events() {
                    events.push(event);
                }
            }
            Ok(RuleOutcome::Revive(Box::new(changes)))
        }
        RuleOp::Command(BattleCommand::Shield(command)) => {
            let changes = crate::engine::manager::shield::execute(managers, command)?;
            if let Some(buff) = &changes.buff {
                for event in buff.events() {
                    events.push(event);
                }
            }
            if let Some(hp) = &changes.hp {
                for event in hp.events() {
                    events.push(event);
                }
            }
            Ok(RuleOutcome::Shield(Box::new(changes)))
        }
        RuleOp::Command(BattleCommand::ExPoint(command)) => {
            let changes = managers.execute_ex_point(command)?;
            for event in changes.events() {
                events.push(event);
            }
            Ok(RuleOutcome::ExPoint(changes))
        }
        RuleOp::Command(BattleCommand::BloodPoolCountAddExPoint(command)) => {
            let Some(changes) =
                crate::engine::skill::buff_act::blood_pool::count_add_ex_point::execute(
                    managers, command,
                )?
            else {
                return Ok(RuleOutcome::StateChanged);
            };
            for event in changes.events() {
                events.push(event);
            }
            Ok(RuleOutcome::ExPoint(changes))
        }
        RuleOp::Command(BattleCommand::Eureka(command)) => {
            let changes = managers.execute_eureka(command)?;
            for event in changes.events() {
                events.push(event);
            }
            Ok(RuleOutcome::Eureka(changes))
        }
        RuleOp::Command(BattleCommand::Gauge(command)) => {
            let change = managers.execute_gauge(command)?;
            for event in change.events() {
                events.push(event);
            }
            Ok(RuleOutcome::Gauge(change))
        }
        RuleOp::Command(BattleCommand::BloodtitheSpend(command)) => {
            let Some(changes) =
                crate::engine::mechanic::bloodtithe::spend::execute(managers, command)?
            else {
                return Ok(RuleOutcome::StateChanged);
            };
            for event in changes
                .gauge
                .events()
                .into_iter()
                .chain(changes.buff.events())
            {
                events.push(event);
            }
            Ok(RuleOutcome::BloodtitheSpend(Box::new(changes)))
        }
        RuleOp::Command(BattleCommand::ThresholdSkill(command)) => {
            let repeats = managers.advance_rule_progress(
                command.owner_uid,
                command.buff_uid,
                command.key,
                command.threshold,
                command.delta,
            );
            Ok(RuleOutcome::ThresholdSkills(
                (0..repeats).map(|_| command.invocation.clone()).collect(),
            ))
        }
        RuleOp::Command(BattleCommand::NuoDiKa(command)) => Ok(RuleOutcome::NuoDiKa(
            managers
                .nuo_di_ka
                .execute(command)
                .map_err(RuleExecutionError::NuoDiKa)?,
        )),
        RuleOp::Command(BattleCommand::Emitter(command)) => {
            Ok(RuleOutcome::Emitter(managers.execute_emitter(command)))
        }
        RuleOp::Command(BattleCommand::Entity(command)) => {
            let changes = managers.execute_entity(command)?;
            for event in changes.events() {
                events.push(event);
            }
            Ok(RuleOutcome::Entity(Box::new(changes)))
        }
        RuleOp::Command(BattleCommand::EntitySkill(command)) => {
            managers.execute_entity_skill(command)?;
            Ok(RuleOutcome::StateChanged)
        }
        RuleOp::Command(BattleCommand::Card(command)) => {
            let changes = managers.execute_card(command)?;
            for event in changes.events() {
                events.push(event);
            }
            Ok(RuleOutcome::Card(Box::new(changes)))
        }
        RuleOp::Command(BattleCommand::BuffPrecast(command)) => {
            let Some(changes) = crate::engine::mechanic::buff_precast::execute(managers, command)?
            else {
                return Ok(RuleOutcome::StateChanged);
            };
            for event in changes
                .buff
                .events()
                .into_iter()
                .chain(changes.card.events())
            {
                events.push(event);
            }
            Ok(RuleOutcome::BuffPrecast(Box::new(changes)))
        }
        RuleOp::Command(BattleCommand::Conduit(command)) => {
            let change = managers.conduit.execute(command)?;
            for event in change.events() {
                events.push(event);
            }
            Ok(RuleOutcome::Conduit(change))
        }
        RuleOp::Command(BattleCommand::Field(command)) => {
            let change = managers.execute_field(command)?;
            for event in change.events() {
                events.push(event);
            }
            Ok(RuleOutcome::Field(change))
        }
        RuleOp::Command(BattleCommand::FieldTransfer(command)) => {
            let changes = crate::engine::mechanic::field_transfer::execute(managers, command)?;
            for buff in &changes.buffs {
                for event in buff.events() {
                    events.push(event);
                }
            }
            for event in changes.field.events() {
                events.push(event);
            }
            Ok(RuleOutcome::FieldTransfer(Box::new(changes)))
        }
        RuleOp::Command(BattleCommand::Shell(command)) => {
            let changes = crate::engine::mechanic::shell::execute(managers, command)?;
            for buff in &changes.buffs {
                for event in buff.events() {
                    events.push(event);
                }
            }
            for event in crate::engine::mechanic::shell::events(&changes) {
                events.push(event);
            }
            Ok(RuleOutcome::Shell(Box::new(changes)))
        }
        RuleOp::Command(BattleCommand::RaspberryCapacity(command)) => {
            let changes =
                crate::engine::skill::buff_act::raspberry::execute_capacity(managers, command)?;
            for event in changes.buff.events() {
                events.push(event);
            }
            for event in changes.hp.events() {
                events.push(event);
            }
            Ok(RuleOutcome::RaspberryCapacity(Box::new(
                CapacityResult::Applied(Box::new(changes)),
            )))
        }
        RuleOp::Command(BattleCommand::RaspberryAddCount(command)) => {
            let Some(changes) =
                crate::engine::skill::buff_act::raspberry::execute_add_count(managers, command)?
            else {
                return Ok(RuleOutcome::StateChanged);
            };
            if let CapacityResult::Applied(applied) = &changes {
                if let Some(change) = applied.ex_point {
                    for event in change.events() {
                        events.push(event);
                    }
                }
                for event in applied.buff.events() {
                    events.push(event);
                }
                for event in applied.hp.events() {
                    events.push(event);
                }
            }
            Ok(RuleOutcome::RaspberryCapacity(Box::new(changes)))
        }
        RuleOp::Command(BattleCommand::Summon(command)) => {
            let changes = managers.execute_summon(command)?;
            for event in changes.events() {
                events.push(event);
            }
            Ok(RuleOutcome::Summon(changes))
        }
        RuleOp::Command(BattleCommand::Upgrade(command)) => {
            let change = managers.execute_upgrade(command)?;
            Ok(RuleOutcome::Upgrade(change))
        }
        RuleOp::Command(BattleCommand::ToughnessRecover(command)) => Ok(managers
            .toughness
            .recover(command)
            .map(RuleOutcome::ToughnessRecovered)
            .unwrap_or(RuleOutcome::StateChanged)),
        RuleOp::Command(BattleCommand::ToughnessRecord(command)) => {
            managers.toughness.record_broken_damage(command);
            Ok(RuleOutcome::StateChanged)
        }
        RuleOp::Skill(_) => Err(RuleExecutionError::UnexpectedSkill),
        RuleOp::BeginSkillAction { lifecycle, cost } => {
            let changes = managers.execute_ex_point(cost)?;
            for event in changes.events() {
                events.push(event);
            }
            match &lifecycle {
                crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action) => {
                    managers
                        .hp
                        .capture_action_start(action.source_uid, action.skill_id);
                    events.push(crate::engine::event::payload::BattleEvent::SkillAction(
                        action.clone(),
                    ));
                }
                _ => return Err(RuleExecutionError::UnexpectedSkill),
            }
            Ok(RuleOutcome::SkillActionStarted {
                lifecycle,
                cost: changes,
            })
        }
        RuleOp::SkillLifecycle(lifecycle) => {
            match &lifecycle {
                crate::engine::skill::action::SkillLifecycle::PhaseCompleted(action) => {
                    if action.phase == crate::engine::skill::action::SkillPhase::Immediate {
                        managers
                            .hp
                            .capture_action_start(action.source_uid, action.skill_id);
                    }
                    events.push(crate::engine::event::payload::BattleEvent::SkillAction(
                        action.clone(),
                    ));
                }
                crate::engine::skill::action::SkillLifecycle::ActionCompleted(action) => events
                    .push(crate::engine::event::payload::BattleEvent::AllyAction(
                        action.clone(),
                    )),
                crate::engine::skill::action::SkillLifecycle::DirectUltimateBodyCompleted {
                    ..
                }
                | crate::engine::skill::action::SkillLifecycle::EmitterAttackStarted(_)
                | crate::engine::skill::action::SkillLifecycle::EmitterSkillEnded { .. } => {}
            }
            Ok(RuleOutcome::SkillLifecycle(lifecycle))
        }
        RuleOp::BuffFeatureMarker {
            target_uid,
            effect_type,
            effect_num,
            buff_act_id,
        } => Ok(RuleOutcome::BuffFeatureMarker(
            crate::engine::manager::buff::BuffMarkerResult {
                target_uid,
                effect_type,
                effect_num,
                buff_act_id,
            },
        )),
        RuleOp::EffectMarker {
            target_uid,
            effect_type,
            effect_num,
            config_effect,
            reserve_id,
            reserve_str,
        } => Ok(RuleOutcome::EffectMarker(
            crate::engine::skill::rule::output::EffectMarker {
                target_uid,
                effect_type,
                effect_num,
                config_effect,
                reserve_id,
                reserve_str,
            },
        )),
        RuleOp::SceneChange { scene_id } => Ok(RuleOutcome::SceneChange { scene_id }),
        RuleOp::BuffActTrigger(trigger) => Ok(RuleOutcome::BuffActTrigger(trigger)),
        RuleOp::BuffActInfoMarker(marker) => Ok(RuleOutcome::BuffActInfoMarker(marker)),
        RuleOp::MarkBuffActFired {
            owner_uid,
            buff_uid,
            key,
        } => {
            managers.mark_buff_act_fired(owner_uid, buff_uid, key);
            Ok(RuleOutcome::StateChanged)
        }
        RuleOp::ModifyActiveSkillTargets { additional_count } => {
            Ok(RuleOutcome::ActiveSkillTargetsModified(additional_count))
        }
    }
}

#[cfg(test)]
mod tests;
