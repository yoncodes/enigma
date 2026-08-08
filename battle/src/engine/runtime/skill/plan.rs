use crate::engine::{
    damage::{AttackPlan, DamageRateTerm, handler as damage},
    manager::{
        BattleManagers,
        buff::{ActiveBuffFeature, BuffCommand, BuffGrant},
        hp::HpCommand,
    },
    runtime::determinism::RoundDeterminism,
    skill::{
        action::{SkillInvocation, SkillModifiers},
        behavior::rate,
        effect::SkillEffectCatalog,
        rule::{
            CommandOrigin, DefinitionKey, RuleDomain,
            output::{BattleCommand, RuleOp},
        },
        target::{TargetContext, TargetPool, TargetRequest, TargetResolver},
    },
};

pub(in crate::engine::runtime) struct SkillExecution {
    pub(super) modifiers: SkillModifiers,
    pub(super) context: TargetContext,
    pub(super) primary_target_uid: Option<i64>,
    pub(super) configured_targets: Option<Vec<i64>>,
    pub(super) configured_additional_targets: Option<Vec<i64>>,
    pub(super) planned_crits: Option<Vec<(i64, bool)>>,
    pub(super) injured_allies: Vec<i64>,
    pub(super) affected_targets: Vec<i64>,
    pub(super) attacked_targets: Vec<i64>,
    pub(super) buff_additions: Vec<(i32, i32)>,
    pub(super) team_injury_count_round: i32,
    team_injury_count_consumed: Option<DefinitionKey>,
    pub(super) activated_additional_damage: Vec<PlannedAdditionalDamage>,
    pub(super) temporary_damage_buffs: Vec<(CommandOrigin, i32)>,
    pub(super) pending_additional_damage: Vec<HpCommand>,
    pending_after_damage_ops: Vec<RuleOp>,
    pending_after_damage_frame_owner: Option<crate::engine::runtime::record::FrameOwner>,
    action_cost: Option<crate::engine::manager::ex_point::ExPointCommand>,
}

impl SkillExecution {
    pub(in crate::engine::runtime) fn new(context: TargetContext) -> Self {
        Self {
            modifiers: SkillModifiers::default(),
            context,
            primary_target_uid: None,
            configured_targets: None,
            configured_additional_targets: None,
            planned_crits: None,
            injured_allies: Vec::new(),
            affected_targets: Vec::new(),
            attacked_targets: Vec::new(),
            buff_additions: Vec::new(),
            team_injury_count_round: 0,
            team_injury_count_consumed: None,
            activated_additional_damage: Vec::new(),
            temporary_damage_buffs: Vec::new(),
            pending_additional_damage: Vec::new(),
            pending_after_damage_ops: Vec::new(),
            pending_after_damage_frame_owner: None,
            action_cost: None,
        }
    }

    pub(in crate::engine::runtime) fn with_modifiers(
        context: TargetContext,
        modifiers: SkillModifiers,
    ) -> Self {
        Self {
            modifiers,
            context,
            primary_target_uid: None,
            configured_targets: None,
            configured_additional_targets: None,
            planned_crits: None,
            injured_allies: Vec::new(),
            affected_targets: Vec::new(),
            attacked_targets: Vec::new(),
            buff_additions: Vec::new(),
            team_injury_count_round: 0,
            team_injury_count_consumed: None,
            activated_additional_damage: Vec::new(),
            temporary_damage_buffs: Vec::new(),
            pending_additional_damage: Vec::new(),
            pending_after_damage_ops: Vec::new(),
            pending_after_damage_frame_owner: None,
            action_cost: None,
        }
    }

    pub(in crate::engine::runtime) fn with_action_cost(
        context: TargetContext,
        cost: Option<crate::engine::manager::ex_point::ExPointCommand>,
    ) -> Self {
        Self {
            action_cost: cost,
            ..Self::new(context)
        }
    }

    pub(super) fn take_action_cost(
        &mut self,
    ) -> Option<crate::engine::manager::ex_point::ExPointCommand> {
        self.action_cost.take()
    }

    pub(in crate::engine::runtime) fn prepare_direct_big(&mut self, additional_moxie: i32) {
        self.context.direct_skill_body = true;
        self.context.additional_moxie = additional_moxie;
    }

    pub(in crate::engine::runtime) fn add_skill_targets(&mut self, count: i32) {
        self.context.additional_skill_target_count += count.max(0);
    }

    pub(super) fn sync_team_injury_count(&mut self, count: i32) {
        self.team_injury_count_round = count.max(0);
        self.context.team_injury_count_round = self.team_injury_count_round;
    }

    pub(super) fn take_team_injury_count_consumption(&mut self) -> Option<DefinitionKey> {
        self.team_injury_count_consumed.take()
    }

    pub(in crate::engine::runtime) fn record_kills(&mut self, count: i32) {
        self.context.action_kill_count += count.max(0);
    }

    pub(in crate::engine::runtime) fn record_guard_breaks(&mut self, count: i32) {
        self.context.action_guard_break_count += count.max(0);
    }

    pub(in crate::engine::runtime) fn record_damage(&mut self, amount: i32) {
        self.context.action_damage_amount += amount.max(0);
        self.context.action_dealt_damage = self.context.action_damage_amount > 0;
    }

    pub(super) fn record_crits(&mut self, count: i32) {
        self.context.action_crit_count += count.max(0);
    }

    pub(in crate::engine::runtime) fn record_injuries(
        &mut self,
        targets: impl IntoIterator<Item = i64>,
    ) {
        for target_uid in targets {
            if !self.injured_allies.contains(&target_uid) {
                self.injured_allies.push(target_uid);
            }
        }
    }

    pub(in crate::engine::runtime) fn record_attacks(
        &mut self,
        targets: impl IntoIterator<Item = i64>,
    ) {
        for target_uid in targets {
            if !self.attacked_targets.contains(&target_uid) {
                self.attacked_targets.push(target_uid);
            }
        }
    }

    pub(in crate::engine::runtime) fn record_buff_additions(
        &mut self,
        additions: impl IntoIterator<Item = (i32, i32)>,
    ) {
        for (buff_id, amount) in additions {
            if amount <= 0 {
                continue;
            }
            if let Some((_, total)) = self
                .buff_additions
                .iter_mut()
                .find(|(active_id, _)| *active_id == buff_id)
            {
                *total = total.saturating_add(amount);
            } else {
                self.buff_additions.push((buff_id, amount));
            }
        }
    }

    pub(in crate::engine::runtime) fn sync_lifecycle_event(
        &self,
        event: &mut crate::engine::skill::action::SkillActionEvent,
    ) {
        event.damage_amount = self.context.action_damage_amount;
        event.kill_count = self.context.action_kill_count;
        event.crit_count = self.context.action_crit_count;
        event.guard_break_count = self.context.action_guard_break_count;
        event.attacked_target_uids = self.attacked_targets.clone();
        event.teammate_injury_count = self.injured_allies.len() as i32;
        event.teammate_injury_count_not_reset = self.injured_allies.len() as i32;
        event.team_injury_count_round = self.team_injury_count_round;
        event.buff_additions.clone_from(&self.buff_additions);
    }

    pub(super) fn take_live_additional_damage(
        &mut self,
        managers: &BattleManagers,
    ) -> Vec<HpCommand> {
        std::mem::take(&mut self.pending_additional_damage)
            .into_iter()
            .filter(|command| managers.hp.current(hp_command_target(*command)) > 0)
            .collect()
    }

    pub(super) fn set_after_damage_ops(
        &mut self,
        ops: Vec<RuleOp>,
        frame_owner: Option<crate::engine::runtime::record::FrameOwner>,
    ) {
        self.pending_after_damage_ops = ops;
        self.pending_after_damage_frame_owner = frame_owner;
    }

    pub(super) fn has_after_damage_ops(&self) -> bool {
        !self.pending_after_damage_ops.is_empty()
    }

    pub(super) fn take_after_damage_ops(
        &mut self,
    ) -> (
        Vec<RuleOp>,
        Option<crate::engine::runtime::record::FrameOwner>,
    ) {
        (
            std::mem::take(&mut self.pending_after_damage_ops),
            self.pending_after_damage_frame_owner.take(),
        )
    }

    pub(in crate::engine::runtime) fn record_targets(
        &mut self,
        targets: impl IntoIterator<Item = i64>,
    ) {
        for target_uid in targets {
            if target_uid != 0 && !self.affected_targets.contains(&target_uid) {
                self.affected_targets.push(target_uid);
            }
        }
    }
}

fn hp_command_target(command: HpCommand) -> i64 {
    match command {
        HpCommand::Damage(value) => value.target_uid,
        HpCommand::Lose(value) => value.target_uid,
        HpCommand::Kill(value) => value.target_uid,
        HpCommand::Heal(value) => value.target_uid,
        HpCommand::SetCurrent(value) => value.target_uid,
        HpCommand::GrantShield(value) => value.target_uid,
        HpCommand::AdjustMax(value) => value.target_uid,
    }
}

pub(super) struct DamageOps {
    pub(super) buff_act_frame_owner: Option<crate::engine::runtime::record::FrameOwner>,
    pub(super) before_damage: Vec<RuleOp>,
    pub(super) damage: Vec<HpCommand>,
    pub(super) additional_damage: Vec<HpCommand>,
    pub(super) after_damage: Vec<RuleOp>,
    pub(super) main_target: Option<i64>,
    pub(super) avoided: Vec<ActiveBuffFeature>,
}
pub(super) struct AdditionalDamageActivation {
    pub(super) additional: PlannedAdditionalDamage,
    pub(super) buff_act_ops: Vec<RuleOp>,
    pub(super) skill_ops: Vec<RuleOp>,
    pub(super) temporary_buff: Option<(CommandOrigin, i32)>,
}

#[derive(Clone)]
pub(super) struct PlannedAdditionalDamage {
    pub(super) feature: ActiveBuffFeature,
    spec: crate::engine::skill::buff_act::additional_damage::AdditionalDamageSpec,
}

fn additional_damage(
    source_uid: i64,
    managers: &BattleManagers,
    execution: &SkillExecution,
    extra_action: bool,
) -> Vec<PlannedAdditionalDamage> {
    let mut planned = execution
        .modifiers
        .additional_damage
        .iter()
        .filter_map(|modifier| {
            crate::engine::skill::buff_act::additional_damage::configured(
                modifier.buff_id,
                source_uid,
                source_uid,
            )
            .map(|(feature, spec)| PlannedAdditionalDamage { feature, spec })
        })
        .filter(|additional| additional.spec.can_apply(managers, extra_action))
        .collect::<Vec<_>>();
    planned.extend(execution.activated_additional_damage.iter().cloned());
    planned
}

pub(super) fn additional_damage_activation(
    invocation: &SkillInvocation,
    managers: &BattleManagers,
    execution: &SkillExecution,
) -> Vec<AdditionalDamageActivation> {
    let extra_action = crate::engine::skill::buff_act::additional_damage::uses_costed_lane(
        execution.context.extra_skill_kind,
    );
    crate::engine::skill::buff_act::additional_damage::active_features(
        managers,
        invocation.plan.source_uid,
    )
    .into_iter()
    .filter_map(|(feature, spec)| {
        if !spec.can_apply(managers, extra_action) {
            return None;
        }
        let additional = PlannedAdditionalDamage { feature, spec };
        let origin = crate::engine::skill::buff_act::feature_command_origin(&additional.feature)?;
        let temporary_buff =
            (additional.spec.temp_buff_id > 0).then_some((origin, additional.spec.temp_buff_id));
        let mut buff_act_ops = Vec::new();
        if additional.spec.source_count_cost > 0 && additional.feature.buff_uid > 0 {
            buff_act_ops.push(RuleOp::Command(
                crate::engine::skill::rule::output::BattleCommand::Buff(
                    crate::engine::manager::buff::BuffCommand::ConsumeCount(
                        crate::engine::manager::buff::BuffConsume {
                            origin,
                            target_uid: additional.feature.owner_uid,
                            selector: crate::engine::manager::buff::BuffSelector::Uid(
                                additional.feature.buff_uid,
                            ),
                            amount: additional.spec.source_count_cost,
                            depleted: crate::engine::manager::buff::DepletedBuff::Remove,
                        },
                    ),
                ),
            ));
        }
        let mut skill_ops = Vec::new();
        if let Some((origin, buff_id)) = temporary_buff {
            skill_ops.push(RuleOp::Command(
                crate::engine::skill::rule::output::BattleCommand::Buff(
                    crate::engine::manager::buff::BuffCommand::Grant(
                        crate::engine::manager::buff::BuffGrant {
                            origin,
                            source_uid: invocation.plan.source_uid,
                            target_uid: invocation.plan.source_uid,
                            buff_id,
                            amount: None,
                            occurrences: 1,
                            child_uid_reservations: 0,
                        },
                    ),
                ),
            ));
        }
        if let Some(op) = crate::engine::skill::buff_act::additional_damage::extra_action_cost_op(
            &additional.feature,
            additional.spec,
            extra_action,
        ) {
            buff_act_ops.push(op);
        }
        Some(AdditionalDamageActivation {
            additional,
            buff_act_ops,
            skill_ops,
            temporary_buff,
        })
    })
    .collect()
}

pub(super) fn damage_ops(
    invocation: &SkillInvocation,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    effect_skill_id: i32,
    determinism: &mut RoundDeterminism,
    execution: &mut SkillExecution,
) -> DamageOps {
    let source_uid = invocation.plan.source_uid;
    let skill_id = invocation.plan.skill_id;
    let rend = crate::engine::skill::buff_act::emitter_rend_target::resolve(
        managers,
        pool,
        source_uid,
        execution.context.emitter_attack_index,
        execution.context.emitter_attack_max,
    );
    let (targets, base_count, behavior_extra_count) = damage_targets(
        invocation,
        managers,
        pool,
        catalog,
        effect_skill_id,
        determinism,
        execution,
        rend.as_ref(),
    );
    let additional_target_order = execution
        .configured_additional_targets
        .as_ref()
        .filter(|targets| !targets.is_empty())
        .cloned()
        .unwrap_or_else(|| targets.clone());
    execution.record_targets(targets.iter().copied());
    let passive_skills = pool
        .entity(source_uid)
        .map(|entity| entity.passive_skills.as_slice())
        .unwrap_or_default();
    let main_target = targets.first().copied();
    let extra_action = crate::engine::skill::buff_act::additional_damage::uses_costed_lane(
        execution.context.extra_skill_kind,
    );
    let additional = additional_damage(source_uid, managers, execution, extra_action)
        .into_iter()
        .filter_map(|additional| {
            crate::engine::skill::buff_act::feature_command_origin(&additional.feature)
                .map(|origin| (additional.feature, additional.spec, origin))
        })
        .collect::<Vec<_>>();
    let active_rate_terms =
        crate::engine::skill::buff_act::life_attack_fix_rate::active_damage_rate_terms(
            source_uid,
            &managers.buff,
            &managers.hp,
        );
    let target_count_damage_bonus =
        crate::engine::skill::buff_act::attr_by_skill_target_count::owner_attribute_delta(
            managers,
            source_uid,
            execution.context.damage_target_count_kind,
            crate::engine::entity::attr::AttrId::DmgBonus,
        );
    let mut damage_commands = Vec::new();
    let mut additional_damage_commands = Vec::new();
    let mut avoided = Vec::new();
    let mut hit_targets = Vec::new();
    let mut crit_count = 0;
    let inherent_assassinate = execution.context.active_skill_assassinate;
    for (index, target_uid) in targets.into_iter().enumerate() {
        if !crate::engine::skill::buff_act::dodge_spec_skill::ignored(managers, source_uid)
            && let Some(feature) = crate::engine::skill::buff_act::dodge_spec_skill::avoidance(
                managers,
                target_uid,
                pool.skill_slot(source_uid, skill_id),
                pool.entity(source_uid)
                    .map(|entity| entity.damage_type)
                    .unwrap_or_default(),
            )
        {
            avoided.push(feature);
            if main_target == Some(target_uid) {
                damage_commands.push(damage::resolve_avoided_attack_command(
                    source_uid,
                    target_uid,
                    CommandOrigin {
                        domain: RuleDomain::Skill,
                        key: DefinitionKey::new(skill_id, "SkillDamage"),
                    },
                ));
            }
            continue;
        }
        let mut target_modifiers = execution.modifiers.clone();
        let mut target_context = execution.context;
        target_context.hit_source_uid = source_uid;
        target_context.hit_target_uid = target_uid;
        target_context.runtime_target_uid = target_uid;
        rate::emit_passive_attack_attributes(
            &mut target_modifiers,
            source_uid,
            skill_id,
            passive_skills,
            rate::RateRuntime {
                effects: catalog,
                managers,
                pool,
                context: target_context,
            },
            determinism,
        );
        let incoming_modifiers = rate::incoming_target_attack_modifiers(
            source_uid,
            target_uid,
            skill_id,
            rate::RateRuntime {
                effects: catalog,
                managers,
                pool,
                context: execution.context,
            },
            determinism,
        );
        target_modifiers.merge(incoming_modifiers);
        execution.team_injury_count_consumed = execution
            .team_injury_count_consumed
            .or(target_modifiers.consume_team_injury_count_round);
        let crit_conversion_rate = target_modifiers.excess_crit_conversion_rate
            + crate::engine::skill::buff_act::crit_rate_alter2::owner_conversion_rate(
                source_uid,
                &managers.buff,
                &managers.hp,
            )
            + crate::engine::skill::buff_act::crit_rate_alter_by_other_buff::owner_conversion_rate(
                source_uid,
                &managers.buff,
                &managers.hp,
            );
        let mut attack_attributes = target_modifiers.attack_attributes.clone();
        if target_count_damage_bonus != 0 {
            attack_attributes.push((
                crate::engine::entity::attr::AttrId::DmgBonus,
                target_count_damage_bonus,
            ));
        }
        // Linked damage keeps the triggering attack's shared damage lane.
        // Inherent Assassination belongs to its attacker, while a target
        // trigger converts the whole incoming attack, including linked hits.
        let mut linked_attack_attributes = attack_attributes.clone();
        let assassination = crate::engine::skill::buff_act::assassination::target_modifier(
            managers,
            source_uid,
            target_uid,
            inherent_assassinate,
        );
        execution.context.active_skill_assassinate |= assassination.assassinate;
        if assassination.final_damage_bonus != 0 {
            attack_attributes.push((
                crate::engine::entity::attr::AttrId::FinalDmgBonus,
                assassination.final_damage_bonus,
            ));
            if assassination.triggered_by_target {
                linked_attack_attributes.push((
                    crate::engine::entity::attr::AttrId::FinalDmgBonus,
                    assassination.final_damage_bonus,
                ));
            }
        }
        for attr_id in [
            crate::engine::entity::attr::AttrId::CriticalDmg,
            crate::engine::entity::attr::AttrId::DmgBonus,
        ] {
            let delta = crate::engine::skill::buff_act::target_attack_attribute_delta(
                managers,
                target_uid,
                extra_action,
                attr_id,
            );
            if delta != 0 {
                attack_attributes.push((attr_id, delta));
            }
        }
        if index >= base_count + behavior_extra_count
            && execution.context.extra_damage_target_final_damage_delta != 0
        {
            attack_attributes.push((
                crate::engine::entity::attr::AttrId::FinalDmgBonus,
                execution.context.extra_damage_target_final_damage_delta,
            ));
        }
        let excess_crit =
            damage::excess_crit_rate(source_uid, target_uid, pool, managers, &attack_attributes);
        let converted = excess_crit * crit_conversion_rate / 1000;
        if crate::engine::diagnostics::enabled(crate::engine::diagnostics::TraceArea::Damage) {
            let crit_chance = damage::crit_chance(source_uid, target_uid, pool, managers)
                + target_modifiers
                    .attack_attributes
                    .iter()
                    .filter_map(|(attr_id, delta)| {
                        (*attr_id == crate::engine::entity::attr::AttrId::CriticalRate)
                            .then_some(*delta)
                    })
                    .sum::<i32>();
            eprintln!(
                "crit chance skill={skill_id} source={source_uid} target={target_uid} chance={crit_chance} excess={excess_crit} conversion_rate={crit_conversion_rate} converted={converted}"
            );
        }
        if converted != 0 {
            attack_attributes.push((crate::engine::entity::attr::AttrId::CriticalDmg, converted));
            linked_attack_attributes
                .push((crate::engine::entity::attr::AttrId::CriticalDmg, converted));
        }
        let rate_terms = target_modifiers
            .rates
            .iter()
            .filter(|modifier| modifier.target_uid == 0 || modifier.target_uid == target_uid)
            .map(|modifier| DamageRateTerm {
                opcode: modifier.opcode,
                rate: modifier.amount.resolve(&managers.gauge),
                career_scaled: modifier.career_scaled,
                composition: modifier.composition,
            })
            .chain(active_rate_terms.iter().copied())
            .chain(rend.as_ref().and_then(|rend| rend.rate_term(target_uid)))
            .collect::<Vec<_>>();
        let linked_rate_terms = rate_terms
            .iter()
            .copied()
            .filter(|term| {
                term.composition == crate::engine::damage::DamageRateComposition::ProducerMultiplier
            })
            .collect::<Vec<_>>();
        let is_crit = execution
            .planned_crits
            .as_ref()
            .and_then(|planned| {
                planned
                    .iter()
                    .find_map(|(uid, crit)| (*uid == target_uid).then_some(*crit))
            })
            .unwrap_or_else(|| {
                determinism.roll_hidden_crit(
                    skill_id,
                    source_uid,
                    target_uid,
                    planned_crit_chance(source_uid, target_uid, managers, pool, execution),
                )
            });
        let main_target = main_target == Some(target_uid);
        if let Some(mut command) = damage::resolve_attack_command(
            &AttackPlan {
                source_uid,
                target_uid,
                skill_id,
                rate: catalog.damage_rate(effect_skill_id),
                rate_terms,
                attack_attributes: attack_attributes.clone(),
                career_ratio_bonus: target_modifiers.career_ratio_bonus,
                attack_career: target_modifiers.attack_career,
                is_conduit: managers.conduit.owns_skill(source_uid, skill_id),
                is_crit,
                assassinate: assassination.assassinate,
                main_target,
                extra_skill_kind: execution.context.extra_skill_kind,
                additional_enabled: false,
                additional_is_crit: None,
            },
            damage::DamageRuntime {
                fight_version: managers.fight_version(),
                pool,
                attributes: &managers.attribute,
                buffs: &managers.buff,
                target_buffs: &managers.buff,
                hp: &managers.hp,
                fields: Some(&managers.field),
                emitter: None,
                team_inspiration: 0,
            },
            CommandOrigin {
                domain: RuleDomain::Skill,
                key: DefinitionKey::new(skill_id, "SkillDamage"),
            },
        ) {
            hit_targets.push(target_uid);
            if let HpCommand::Damage(damage) = &mut command {
                damage.ignore_riposte = target_modifiers.ignore_riposte;
            }
            crit_count += i32::from(is_crit);
            damage_commands.extend(crate::engine::skill::buff_act::absorb_hurt::route(
                managers, pool, command,
            ));
        }
        for (_, additional, origin) in &additional {
            let additional_is_crit = determinism.roll_additional_crit(
                skill_id,
                source_uid,
                additional.credited_source_uid,
                target_uid,
                damage::crit_chance(additional.credited_source_uid, target_uid, pool, managers),
            );
            let additional_attributes = linked_attack_attributes
                .iter()
                .copied()
                .filter(|(attr, _)| {
                    !matches!(
                    attr,
                    crate::engine::entity::attr::AttrId::UltimateMight
                        | crate::engine::entity::attr::AttrId::IncantationMight
                        | crate::engine::entity::attr::AttrId::UltimateMightMultiplier
                        | crate::engine::entity::attr::AttrId::IncantationSkillUltMightMultiplier
                        | crate::engine::entity::attr::AttrId::IncantationMightMultiplier
                )
                })
                .collect::<Vec<_>>();
            if let Some(mut command) = damage::resolve_additional_damage_command(
                damage::DamageRequest {
                    source_uid: additional.credited_source_uid,
                    target_uid,
                    skill_id,
                    rate: additional.rate(
                        additional_target_order.first() == Some(&target_uid),
                        extra_action,
                    ),
                    rate_terms: &linked_rate_terms,
                    // The credited source owns base ATK/crit and persistent buffs. The linked
                    // hit inherits the triggering attack's regular damage lane, but not its
                    // incantation/ultimate-specific might lane.
                    attack_attributes: &additional_attributes,
                    career_ratio_bonus: target_modifiers.career_ratio_bonus,
                    attack_career: target_modifiers.attack_career,
                    is_conduit: false,
                    is_crit: additional_is_crit,
                    extra_skill_kind: execution.context.extra_skill_kind,
                },
                damage::DamageRuntime {
                    fight_version: managers.fight_version(),
                    pool,
                    attributes: &managers.attribute,
                    buffs: &managers.buff,
                    target_buffs: &managers.buff,
                    hp: &managers.hp,
                    fields: Some(&managers.field),
                    emitter: None,
                    team_inspiration: 0,
                },
                additional.attack_replacement(managers),
                additional.credited_source_uid,
                assassination.triggered_by_target,
                *origin,
            ) {
                if let HpCommand::Damage(damage) = &mut command {
                    damage.ignore_riposte = target_modifiers.ignore_riposte;
                }
                additional_damage_commands.push((
                    target_uid,
                    crate::engine::skill::buff_act::absorb_hurt::route(managers, pool, command),
                ));
            }
        }
    }
    let mut ordered_additional_damage = Vec::new();
    for target_uid in additional_target_order {
        while let Some(index) = additional_damage_commands
            .iter()
            .position(|(uid, _)| *uid == target_uid)
        {
            ordered_additional_damage.extend(additional_damage_commands.remove(index).1);
        }
    }
    if execution.planned_crits.is_none() {
        execution.record_crits(crit_count);
    }
    let mut after_damage = rend
        .as_ref()
        .map(|rend| rend.after_damage_rule_ops())
        .unwrap_or_default();
    after_damage.extend(
        execution
            .modifiers
            .after_damage_buffs
            .iter()
            .flat_map(|modifier| {
                hit_targets.iter().map(|target_uid| {
                    RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                        origin: modifier.origin,
                        source_uid,
                        target_uid: *target_uid,
                        buff_id: modifier.buff_id,
                        amount: Some(modifier.amount),
                        occurrences: 1,
                        child_uid_reservations: 0,
                    })))
                })
            }),
    );
    DamageOps {
        buff_act_frame_owner: rend.as_ref().and_then(|rend| rend.frame_owner()),
        before_damage: rend
            .as_ref()
            .map(|rend| rend.before_damage_rule_ops())
            .unwrap_or_default(),
        damage: damage_commands,
        additional_damage: ordered_additional_damage,
        after_damage,
        main_target,
        avoided,
    }
}

#[allow(clippy::too_many_arguments)]
fn damage_targets(
    invocation: &SkillInvocation,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    effect_skill_id: i32,
    determinism: &mut RoundDeterminism,
    execution: &mut SkillExecution,
    rend: Option<&crate::engine::skill::buff_act::emitter_rend_target::EmitterRendPlan>,
) -> (Vec<i64>, usize, usize) {
    let source_uid = invocation.plan.source_uid;
    let skill_id = invocation.plan.skill_id;
    let request = TargetRequest {
        code: catalog.logic_target(effect_skill_id),
        raw: Vec::new(),
    };
    if let Some(rend) = rend {
        let targets = rend.targets();
        let base_count = targets.len();
        execution.configured_targets = Some(targets.clone());
        return (targets, base_count, 0);
    }
    let mut targets = if execution.modifiers.redirected_damage_targets.is_empty() {
        execution.configured_targets.clone().unwrap_or_else(|| {
            TargetResolver::resolve_action_targets(
                &request,
                skill_id,
                source_uid,
                pool,
                determinism,
                Some(managers),
                execution.context,
            )
        })
    } else {
        execution.modifiers.redirected_damage_targets.clone()
    };
    let base_count = catalog
        .target_limit(effect_skill_id)
        .max(crate::engine::skill::target::request::target_count(request.code).max(0) as usize)
        .max(1);
    let behavior_extra_count = execution.context.additional_skill_target_count.max(0) as usize;
    let extra_count =
        behavior_extra_count + execution.context.extra_damage_target_count.max(0) as usize;
    if extra_count > 0 {
        let total = base_count + extra_count;
        let extras = pool
            .enemies(source_uid, false)
            .iter()
            .map(|target| target.uid)
            .filter(|uid| !targets.contains(uid))
            .take(total.saturating_sub(targets.len()))
            .collect::<Vec<_>>();
        targets.extend(extras);
        targets.truncate(total);
    }
    execution.configured_targets = Some(targets.clone());
    (targets, base_count, behavior_extra_count)
}

pub(super) fn plan_crits(
    invocation: &SkillInvocation,
    managers: &BattleManagers,
    pool: &TargetPool,
    catalog: &SkillEffectCatalog,
    effect_skill_id: i32,
    determinism: &mut RoundDeterminism,
    execution: &mut SkillExecution,
) {
    let source_uid = invocation.plan.source_uid;
    let skill_id = invocation.plan.skill_id;
    let rend = crate::engine::skill::buff_act::emitter_rend_target::resolve(
        managers,
        pool,
        source_uid,
        execution.context.emitter_attack_index,
        execution.context.emitter_attack_max,
    );
    let (targets, _, _) = damage_targets(
        invocation,
        managers,
        pool,
        catalog,
        effect_skill_id,
        determinism,
        execution,
        rend.as_ref(),
    );
    let planned = targets
        .into_iter()
        .map(|target_uid| {
            let is_crit = determinism.roll_hidden_crit(
                skill_id,
                source_uid,
                target_uid,
                planned_crit_chance(source_uid, target_uid, managers, pool, execution),
            );
            (target_uid, is_crit)
        })
        .collect::<Vec<_>>();
    execution.record_crits(planned.iter().filter(|(_, is_crit)| *is_crit).count() as i32);
    execution.planned_crits = Some(planned);
}

fn planned_crit_chance(
    source_uid: i64,
    target_uid: i64,
    managers: &BattleManagers,
    pool: &TargetPool,
    execution: &SkillExecution,
) -> i32 {
    let extra_action = crate::engine::skill::condition::extra::skill_kind_from_is_extra(
        execution.context.extra_skill_kind,
    )
    .is_some_and(|kind| kind.is_extra_action());
    let field_forces_critical = extra_action && {
        let active_features = managers.buff.active_features(&managers.hp);
        crate::engine::skill::buff_act::must_crit_and_fix_temp_attr::forces_critical(
            &active_features,
            source_uid,
            true,
        )
    };
    if execution.modifiers.force_critical || field_forces_critical {
        return 1000;
    }
    damage::crit_chance(source_uid, target_uid, pool, managers)
        + execution
            .modifiers
            .attack_attributes
            .iter()
            .filter(|(attr, _)| *attr == crate::engine::entity::attr::AttrId::CriticalRate)
            .map(|(_, delta)| *delta)
            .sum::<i32>()
}
