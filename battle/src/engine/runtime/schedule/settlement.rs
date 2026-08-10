use super::*;
use crate::engine::manager::card::enchant::collect_round_end_current_hp_losses;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementSide {
    Attacker,
    Defender,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettlementStep {
    OwnerEvent(EventKind),
    Settlement,
    ResetEureka,
}

const ATTACKER_SETTLEMENT: &[SettlementStep] = &[
    SettlementStep::OwnerEvent(EventKind::RoundEndEntitySettlement),
    SettlementStep::Settlement,
    SettlementStep::OwnerEvent(EventKind::RoundEndAfterSettlement),
    SettlementStep::ResetEureka,
];

const DEFENDER_SETTLEMENT: &[SettlementStep] = &[
    SettlementStep::Settlement,
    SettlementStep::OwnerEvent(EventKind::RoundEndEntitySettlement),
    SettlementStep::OwnerEvent(EventKind::RoundEndAfterSettlement),
    SettlementStep::ResetEureka,
];

pub struct EntitySettlement {
    pub output: DrainResult,
    pub settled_buffs: Vec<crate::engine::event::payload::BuffChangeEvent>,
}

pub fn run_attacker_round_end(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> Result<DrainResult, DrainError> {
    let entity_owners = pool
        .attacker_main
        .iter()
        .filter(|entity| managers.hp.current(entity.uid) > 0)
        .map(|entity| entity.uid)
        .collect::<Vec<_>>();
    let mut result = run_no_action_round(
        managers,
        pool,
        catalog,
        determinism,
        context,
        &entity_owners,
    )?;
    let mut owners = entity_owners;
    owners.extend(pool.assist_boss(crate::engine::fight::rules::ATTACKER_SIDE_UID));
    owners.push(crate::engine::fight::rules::ATTACKER_SIDE_UID);
    append(
        &mut result,
        drain::run_grouped_owner_event(
            managers,
            pool,
            catalog,
            determinism,
            context,
            BattleEvent::Kind(EventKind::SmallRoundEnd),
            &owners,
            drain::ReactionLane::Skills,
        )?,
    );
    append(
        &mut result,
        run_duration_advances_for_event(
            managers,
            pool,
            catalog,
            determinism,
            context,
            EventKind::SmallRoundEnd,
            &owners,
        )?,
    );
    append(
        &mut result,
        drain::run_group_event(
            managers,
            pool,
            catalog,
            determinism,
            context,
            BattleEvent::Kind(EventKind::RoundEnd),
            drain::ReactionLane::BuffActs,
            Some(&owners),
        )?,
    );
    append(
        &mut result,
        drain::run_group_event(
            managers,
            pool,
            catalog,
            determinism,
            context,
            BattleEvent::Kind(EventKind::RoundEnd),
            drain::ReactionLane::Skills,
            Some(&owners),
        )?,
    );
    let mut settlement_owners = owners;
    settlement_owners.push(crate::engine::manager::emitter::UID);
    append(
        &mut result,
        run_entity_settlement(
            managers,
            pool,
            catalog,
            determinism,
            context,
            &settlement_owners,
            SettlementSide::Attacker,
        )?
        .output,
    );
    append(
        &mut result,
        run_card_enchant_round_end(managers, pool, catalog, determinism, context)?,
    );
    Ok(result)
}

fn run_card_enchant_round_end(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> Result<DrainResult, DrainError> {
    let battle_catalog = managers.catalog();
    let losses = collect_round_end_current_hp_losses(managers.card.hand(), |enchant_id| {
        battle_catalog.card_enchant_current_hp_loss_permille(enchant_id)
    });
    let commands = losses
        .into_iter()
        .filter_map(|loss| {
            let current_hp = managers.hp.current(loss.owner_uid);
            let amount = (i64::from(current_hp) * i64::from(loss.permille) / 1000)
                .clamp(0, i64::from(i32::MAX)) as i32;
            (amount > 0).then_some(crate::engine::manager::hp::HpCommand::Lose(
                crate::engine::manager::hp::HpLoss {
                    origin: crate::engine::skill::rule::CommandOrigin {
                        domain: crate::engine::skill::rule::RuleDomain::Lifecycle,
                        key: crate::engine::skill::rule::DefinitionKey::new(0, "ScaldingRoundEnd"),
                    },
                    source_uid: loss.owner_uid,
                    target_uid: loss.owner_uid,
                    amount,
                    config_effect: 0,
                    hurt: Some(crate::engine::manager::hp::HurtInfoData {
                        from_uid: loss.owner_uid,
                        is_crit: false,
                        career_restraint: false,
                        reduce_hp: -amount,
                        effect_id: 0,
                        skill_id: 0,
                        damage_from: crate::engine::manager::hp::HurtDamageFromType::None,
                        buff_act_id: 0,
                        buff_uid: 0,
                        hurt_effect_type:
                            sonettobuf::effect_type_enum::EffectType::Enchantburndamage as i32,
                        display_amount: Some(amount),
                    }),
                },
            ))
        })
        .collect::<Vec<_>>();
    if commands.is_empty() {
        return Ok(DrainResult::default());
    }
    drain::run_command_group(
        managers,
        pool,
        catalog,
        determinism,
        context,
        [RuleOp::Command(BattleCommand::HpBatch(commands))],
    )
}

pub fn run_finished_attacker_settlement(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> Result<DrainResult, DrainError> {
    let mut owners = pool
        .attacker_main
        .iter()
        .filter(|entity| managers.hp.current(entity.uid) > 0)
        .map(|entity| entity.uid)
        .collect::<Vec<_>>();
    let mut result = run_duration_advances_for_event(
        managers,
        pool,
        catalog,
        determinism,
        context,
        EventKind::SmallRoundEnd,
        &owners,
    )?;
    owners.push(crate::engine::manager::emitter::UID);
    append(
        &mut result,
        run_entity_settlement(
            managers,
            pool,
            catalog,
            determinism,
            context,
            &owners,
            SettlementSide::Attacker,
        )?
        .output,
    );
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn run_duration_advances_for_event(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    event: EventKind,
    owner_uids: &[i64],
) -> Result<DrainResult, DrainError> {
    let ops = effect_time::duration_stages_for_event(event).map(|take_stage| {
        RuleOp::Command(BattleCommand::Buff(BuffCommand::AdvanceDuration(
            BuffDurationAdvance::new(take_stage, owner_uids.to_vec(), None)
                .expect("event duration stage is registered"),
        )))
    });
    drain::run_command_group(managers, pool, catalog, determinism, context, ops)
}

pub fn run_after_ai_round_end(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> Result<DrainResult, DrainError> {
    let (mut result, settled_buffs) =
        run_defender_settlement(managers, pool, catalog, determinism, context)?;
    append(
        &mut result,
        run_round_end_transition(managers, pool, catalog, determinism, context, settled_buffs)?,
    );
    Ok(result)
}

fn run_defender_settlement(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
) -> Result<
    (
        DrainResult,
        Vec<crate::engine::event::payload::BuffChangeEvent>,
    ),
    DrainError,
> {
    let mut owners = pool
        .defender_main
        .iter()
        .filter(|entity| managers.hp.current(entity.uid) > 0)
        .map(|entity| entity.uid)
        .collect::<Vec<_>>();
    owners.extend(pool.assist_boss(crate::engine::fight::rules::DEFENDER_SIDE_UID));
    owners.push(crate::engine::fight::rules::DEFENDER_SIDE_UID);
    let mut result = drain::run_group_event(
        managers,
        pool,
        catalog,
        determinism,
        context,
        BattleEvent::Kind(EventKind::RoundEnd),
        drain::ReactionLane::BuffActs,
        Some(&owners),
    )?;
    append(
        &mut result,
        drain::run_grouped_owner_event(
            managers,
            pool,
            catalog,
            determinism,
            context,
            BattleEvent::Kind(EventKind::SmallRoundEnd),
            &owners,
            drain::ReactionLane::Skills,
        )?,
    );
    append(
        &mut result,
        drain::run_group_event(
            managers,
            pool,
            catalog,
            determinism,
            context,
            BattleEvent::Kind(EventKind::RoundEnd),
            drain::ReactionLane::Skills,
            Some(&owners),
        )?,
    );
    let settlement = run_entity_settlement(
        managers,
        pool,
        catalog,
        determinism,
        context,
        &owners,
        SettlementSide::Defender,
    )?;
    let settled_buffs = settlement.settled_buffs;
    append(&mut result, settlement.output);
    Ok((result, settled_buffs))
}

fn run_round_end_transition(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    settled_buffs: Vec<crate::engine::event::payload::BuffChangeEvent>,
) -> Result<DrainResult, DrainError> {
    let mut result = DrainResult::default();
    push_cue(&mut result.frames, RoundCue::SmallRoundEnd { team_type: 1 });
    push_cue(&mut result.frames, RoundCue::ClearUniversalCard);
    append(
        &mut result,
        run_round_end_final_settlement(
            managers,
            pool,
            catalog,
            determinism,
            context,
            settled_buffs,
        )?,
    );
    Ok(result)
}

pub fn run_entity_settlement(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    owner_uids: &[i64],
    side: SettlementSide,
) -> Result<EntitySettlement, DrainError> {
    let schedule = match side {
        SettlementSide::Attacker => ATTACKER_SETTLEMENT,
        SettlementSide::Defender => DEFENDER_SETTLEMENT,
    };
    let mut result = DrainResult::default();
    let mut settlement_changes = Vec::new();
    for step in schedule {
        let next = match step {
            SettlementStep::OwnerEvent(kind) => {
                let event = BattleEvent::Kind(*kind);
                if *kind == EventKind::RoundEndAfterSettlement {
                    let mut result = drain::run_grouped_owner_event(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        context,
                        event.clone(),
                        owner_uids,
                        drain::ReactionLane::BuffActs,
                    )?;
                    let mut skills = drain::run_grouped_owner_event(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        context,
                        event.clone(),
                        owner_uids,
                        drain::ReactionLane::Skills,
                    )?;
                    if skills.events.first() == Some(&event) {
                        skills.events.remove(0);
                    }
                    append(&mut result, skills);
                    result
                } else {
                    let mut result = drain::run_grouped_owner_event(
                        managers,
                        pool,
                        catalog,
                        determinism,
                        context,
                        event.clone(),
                        owner_uids,
                        drain::ReactionLane::Skills,
                    )?;
                    if *kind == EventKind::RoundEndEntitySettlement {
                        let mut buff_acts = drain::run_grouped_owner_event(
                            managers,
                            pool,
                            catalog,
                            determinism,
                            context,
                            event.clone(),
                            owner_uids,
                            drain::ReactionLane::BuffActs,
                        )?;
                        if buff_acts.events.first() == Some(&event) {
                            buff_acts.events.remove(0);
                        }
                        append(&mut result, buff_acts);
                    }
                    result
                }
            }
            SettlementStep::Settlement => {
                let settlement = run_round_end_settlement(
                    managers,
                    pool,
                    catalog,
                    determinism,
                    context,
                    owner_uids,
                )?;
                settlement_changes = settlement
                    .events
                    .iter()
                    .filter_map(|event| match event {
                        BattleEvent::BuffChanged(change) | BattleEvent::BuffRemoved(change)
                            if change.after_amount < change.before_amount =>
                        {
                            Some(*change)
                        }
                        _ => None,
                    })
                    .collect();
                settlement
            }
            SettlementStep::ResetEureka => drain::run(
                managers,
                pool,
                catalog,
                determinism,
                context,
                [RuleOp::Command(BattleCommand::Eureka(
                    EurekaCommand::ResetRound {
                        owner_uids: owner_uids.to_vec(),
                    },
                ))],
            )?,
        };
        append(&mut result, next);
    }
    Ok(EntitySettlement {
        output: result,
        settled_buffs: settlement_changes,
    })
}

pub fn run_round_end_final_settlement(
    managers: &mut BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    determinism: &mut RoundDeterminism,
    context: TargetContext,
    settled_buffs: Vec<crate::engine::event::payload::BuffChangeEvent>,
) -> Result<DrainResult, DrainError> {
    let event = BattleEvent::BuffsSettled(settled_buffs);
    let owner_uids = final_settlement_owner_order(pool, managers);
    let mut result = drain::run_grouped_owner_event(
        managers,
        pool,
        catalog,
        determinism,
        context,
        event.clone(),
        &owner_uids,
        drain::ReactionLane::BuffActs,
    )?;
    let mut skills = drain::run_group_event(
        managers,
        pool,
        catalog,
        determinism,
        context,
        event.clone(),
        drain::ReactionLane::Skills,
        None,
    )?;
    if skills.events.first() == Some(&event) {
        skills.events.remove(0);
    }
    append(&mut result, skills);
    Ok(result)
}

fn final_settlement_owner_order(pool: &TargetPool, managers: &BattleManagers) -> Vec<i64> {
    let mut owners = pool
        .entities()
        .filter(|entity| managers.hp.current(entity.uid) > 0)
        .map(|entity| entity.uid)
        .collect::<Vec<_>>();
    for owner_uid in [
        crate::engine::fight::rules::ATTACKER_SIDE_UID,
        crate::engine::manager::emitter::UID,
        crate::engine::fight::rules::DEFENDER_SIDE_UID,
    ] {
        if !owners.contains(&owner_uid) {
            owners.push(owner_uid);
        }
    }
    owners
}
