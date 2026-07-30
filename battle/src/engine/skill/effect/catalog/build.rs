use super::parse::{
    RawSlot, monster_model_skills, numeric_ids, parse_slot, parse_target, row_slots, rule_issue,
};
use super::*;

impl SkillEffectCatalog {
    pub fn from_game_db(db: &GameDB) -> Self {
        let mut catalog = Self::default();

        for row in db.skill_effect.all() {
            catalog.insert_configured_effect(db, row);
        }

        for skill in db.skill.all() {
            if skill.skill_effect != 0 {
                catalog.insert_alias(skill.id, skill.skill_effect);
            }
        }
        catalog.reinforced_skills.extend(
            db.hero_upgrade_breaklevel
                .iter()
                .map(|row| (row.skill_id, row.upgrade_skill_id)),
        );

        catalog
    }

    /// Compiles only the skill and effect closure reachable from one fight roster.
    pub fn from_fight(db: &GameDB, fight: &Fight) -> Self {
        let mut skills = Vec::new();
        let mut buffs = Vec::new();
        for team in [fight.attacker.as_ref(), fight.defender.as_ref()]
            .into_iter()
            .flatten()
        {
            for entity in team
                .entitys
                .iter()
                .chain(&team.sub_entitys)
                .chain(&team.sp_entitys)
                .chain(&team.sp_fight_entities)
                .chain(team.assist_boss.iter())
                .chain(team.emitter.iter())
                .chain(team.player_entity.iter())
                .chain(team.vorpalith.iter())
            {
                skills.extend(entity.skill_group1.iter().copied());
                skills.extend(entity.skill_group2.iter().copied());
                skills.extend(entity.passive_skill.iter().copied());
                skills.extend(entity.ex_skill);
                buffs.extend(
                    entity
                        .buffs
                        .iter()
                        .chain(&entity.no_effect_buffs)
                        .filter_map(|buff| buff.buff_id),
                );
            }
            skills.extend(team.skill_infos.iter().filter_map(|skill| skill.skill_id));
            skills.extend(
                team.assist_boss_info
                    .iter()
                    .flat_map(|info| &info.skills)
                    .filter_map(|skill| skill.skill_id),
            );
        }
        if let Some(battle) = crate::engine::fight::configured_battle(fight) {
            for rule_id in
                numeric_ids(&battle.addition_rule).chain(numeric_ids(&battle.hidden_rule))
            {
                if let Some(rule) = db.rule.get(rule_id) {
                    skills
                        .extend(numeric_ids(&rule.effect).filter(|id| db.skill.get(*id).is_some()));
                }
            }
        }
        let catalog = Self::from_roots(db, skills, buffs);
        catalog.warn_unsupported(db);
        catalog
    }

    pub fn extend_entities_and_warn<'a>(
        &mut self,
        db: &GameDB,
        entities: impl IntoIterator<Item = &'a sonettobuf::FightEntityInfo>,
    ) {
        let mut skills = Vec::new();
        let mut buffs = Vec::new();
        for entity in entities {
            skills.extend(entity.skill_group1.iter().copied());
            skills.extend(entity.skill_group2.iter().copied());
            skills.extend(entity.passive_skill.iter().copied());
            skills.extend(entity.ex_skill);
            buffs.extend(
                entity
                    .buffs
                    .iter()
                    .chain(&entity.no_effect_buffs)
                    .filter_map(|buff| buff.buff_id),
            );
        }
        self.extend_roots(db, skills, buffs);
        self.warn_unsupported(db);
    }

    pub fn from_roots(
        db: &GameDB,
        skills: impl IntoIterator<Item = i32>,
        buffs: impl IntoIterator<Item = i32>,
    ) -> Self {
        let mut catalog = Self::default();
        catalog.extend_roots(db, skills, buffs);
        catalog
    }

    /// Extends the catalog from explicit skill and buff roots through config links.
    pub fn extend_roots(
        &mut self,
        db: &GameDB,
        skills: impl IntoIterator<Item = i32>,
        buffs: impl IntoIterator<Item = i32>,
    ) {
        let mut skills = skills
            .into_iter()
            .filter(|id| *id > 0)
            .collect::<VecDeque<_>>();
        let mut buffs = buffs
            .into_iter()
            .filter(|id| *id > 0)
            .collect::<VecDeque<_>>();
        let mut models = VecDeque::new();
        let mut seen_skills = HashSet::new();
        let mut seen_buffs = HashSet::new();
        let mut seen_models = HashSet::new();

        while !skills.is_empty() || !buffs.is_empty() || !models.is_empty() {
            while let Some(skill_id) = skills.pop_front() {
                if !seen_skills.insert(skill_id) {
                    continue;
                }
                if let Some(reinforced) = db
                    .hero_upgrade_breaklevel
                    .iter()
                    .find(|row| row.skill_id == skill_id)
                    .map(|row| row.upgrade_skill_id)
                    .filter(|id| *id > 0)
                {
                    self.reinforced_skills.insert(skill_id, reinforced);
                    skills.push_back(reinforced);
                }
                let effect_id = db
                    .skill
                    .get(skill_id)
                    .map(|skill| skill.skill_effect)
                    .filter(|id| *id != 0)
                    .unwrap_or(skill_id);
                if effect_id != skill_id {
                    self.insert_alias(skill_id, effect_id);
                }
                let Some(row) = db.skill_effect.get(effect_id) else {
                    continue;
                };
                if self.effects.contains_key(&effect_id) {
                    continue;
                }
                let references = self.insert_configured_effect(db, row);
                skills.extend(references.skills);
                buffs.extend(references.buffs);
                models.extend(references.models);
            }
            while let Some(buff_id) = buffs.pop_front() {
                if !seen_buffs.insert(buff_id) {
                    continue;
                }
                self.reachable_buffs.insert(buff_id);
                let Some(buff) = db.skill_buff.get(buff_id) else {
                    continue;
                };
                for raw in buff.features.split('|') {
                    let values = crate::engine::entity::skill::split_ids(raw);
                    let Some(&feature_id) = values.first() else {
                        continue;
                    };
                    let Some(act) = db.buff_act.get(feature_id) else {
                        if db.skill_buff.get(feature_id).is_some() {
                            buffs.push_back(feature_id);
                        }
                        continue;
                    };
                    use crate::engine::skill::buff_act::registry::BuffActKind;
                    match crate::engine::skill::buff_act::registry::kind(act.id, &act.r#type) {
                        Some(BuffActKind::SubBuff) => buffs.extend(values.get(1).copied()),
                        Some(BuffActKind::MasterHalo) => {
                            buffs.extend(values.get(2).copied().filter(|id| *id > 0))
                        }
                        Some(BuffActKind::AddBuffToEnter) => buffs.extend(
                            crate::engine::skill::buff_act::add_buff_to_enter::referenced_buff(
                                &values[1..],
                            ),
                        ),
                        Some(BuffActKind::TransferEnergyBuff) => buffs.extend(
                            crate::engine::skill::buff_act::transfer_energy_buff::referenced_buff(
                                &values[1..],
                            ),
                        ),
                        Some(BuffActKind::AddPassiveSkills)
                        | Some(BuffActKind::AddSpTempCard)
                        | Some(BuffActKind::CastChannel)
                        | Some(BuffActKind::SpecialCountCastChannel) => {
                            skills.extend(values.get(1).copied())
                        }
                        Some(BuffActKind::AddCardCastChannel) => skills.extend(
                            crate::engine::skill::buff_act::add_card_cast_channel::referenced_skill(
                                &values[1..],
                            ),
                        ),
                        Some(BuffActKind::BeatBack) => skills.extend(
                            crate::engine::skill::buff_act::riposte::holder_skill(&values[1..]),
                        ),
                        Some(BuffActKind::CardNotCalSize) => skills.extend(
                            values
                                .iter()
                                .skip(1)
                                .copied()
                                .filter(|id| db.skill.get(*id).is_some()),
                        ),
                        Some(BuffActKind::AdrenalineAddCard) => skills.extend(
                            raw.split('#')
                                .nth(2)
                                .into_iter()
                                .flat_map(|ids| ids.split(','))
                                .filter_map(|id| id.parse::<i32>().ok()),
                        ),
                        Some(BuffActKind::NuoDiKaCastChannel) => skills.extend(
                            crate::engine::skill::buff_act::nuo_di_ka_cast_channel::referenced_skills(
                                &values[1..],
                            ),
                        ),
                        Some(BuffActKind::HeatScaleUseSkill) => skills.extend(
                            crate::engine::mechanic::heat_scale::referenced_skills(raw),
                        ),
                        Some(BuffActKind::PaperCircleContinueChannel) => skills.extend(
                            crate::engine::skill::buff_act::paper_circle_continue_channel::referenced_skill(raw),
                        ),
                        Some(BuffActKind::BloodValueUseSkill) => {
                            skills.extend(values.get(3).copied())
                        }
                        Some(
                            BuffActKind::UseSkillToEnemy
                            | BuffActKind::ConsumeBuffContinueChannel
                            | BuffActKind::ConsumeBuffAddBuffContinueChannel
                            | BuffActKind::MonitorContinueChannel,
                        ) => skills.extend(
                            crate::engine::skill::buff_act::use_skill::linked_for(
                                0,
                                act.id,
                                &act.r#type,
                                &values[1..],
                            )
                            .map(|request| request.skill_id),
                        ),
                        Some(BuffActKind::EmitterTag) => skills.extend(
                            crate::engine::mechanic::impromptu::ImpromptuDefinition::from_config()
                                .map(|definition| definition.skill_id()),
                        ),
                        Some(BuffActKind::BeatBackDependOnAttackMe) => {
                            skills.extend(values.iter().skip(1).take(2).copied())
                        }
                        _ => {}
                    }
                }
            }
            while let Some(model_id) = models.pop_front() {
                if !seen_models.insert(model_id) {
                    continue;
                }
                skills.extend(monster_model_skills(db, model_id));
            }
        }
    }

    pub fn extend_roots_and_warn(
        &mut self,
        db: &GameDB,
        skills: impl IntoIterator<Item = i32>,
        buffs: impl IntoIterator<Item = i32>,
    ) {
        let existing = self.effects.keys().copied().collect::<HashSet<_>>();
        let existing_buffs = self.reachable_buffs.clone();
        self.extend_roots(db, skills, buffs);
        self.warn_unsupported_matching(|effect_id| !existing.contains(&effect_id));
        self.warn_unsupported_buffs(db, |buff_id| !existing_buffs.contains(&buff_id));
    }

    fn insert_configured_effect(
        &mut self,
        db: &GameDB,
        row: &config::skill_effect::SkillEffect,
    ) -> crate::engine::skill::rule::RuleReferences {
        let mut references = crate::engine::skill::rule::RuleReferences::default();
        let mut slots = Vec::new();
        for (index, (behavior, target, condition, condition_target, limit, round_limit)) in
            row_slots(row).into_iter().enumerate()
        {
            if behavior.trim().is_empty() {
                continue;
            }
            if let Some(slot) = parse_slot(
                db,
                RawSlot {
                    behavior,
                    target,
                    condition,
                    condition_target,
                    logic_target: &row.logic_target,
                    limit,
                    round_limit,
                },
            ) {
                if let Some(definition) =
                    crate::engine::skill::behavior::registry::find(&slot.behavior)
                {
                    let found = (definition.references)(&slot.behavior);
                    references.skills.extend(found.skills);
                    references.buffs.extend(found.buffs);
                    references.models.extend(found.models);
                }
                slots.push(slot);
            } else {
                self.issues.entry(row.id).or_default().push(rule_issue(
                    db,
                    row.id,
                    index as u8 + 1,
                    behavior,
                ));
            }
        }
        self.insert(ParsedSkillEffect {
            skill_id: row.id,
            slots,
        });
        self.insert_effect_tag(row.id, row.effect_tag);
        self.insert_logic_target(row.id, parse_target(&row.logic_target).code);
        self.insert_damage_rate(row.id, row.damage_rate);
        self.skill_types.insert(row.id, row.r#type);
        self.skill_effect_types
            .insert(row.id, row.skill_effect_type);
        self.extra_kinds.insert(row.id, row.is_extra);
        self.big_skill_points.insert(row.id, row.big_skill_point);
        self.big_skills.insert(row.id, row.is_big_skill != 0);
        self.target_limits.insert(row.id, row.target_limit);
        references
    }

    fn warn_unsupported(&self, db: &GameDB) {
        self.warn_unsupported_matching(|_| true);
        self.warn_unsupported_buffs(db, |_| true);
    }

    fn warn_unsupported_matching(&self, include: impl Fn(i32) -> bool) {
        for (&effect_id, issues) in &self.issues {
            if !include(effect_id) {
                continue;
            }
            for issue in issues {
                tracing::warn!(
                    skill_effect_id = issue.effect_id,
                    slot = issue.slot,
                    behavior_opcode = ?issue.opcode,
                    behavior_type = ?issue.type_name,
                    reason = ?issue.reason,
                    "unsupported behavior in current battle"
                );
            }
        }

        for effect in self.effects.values() {
            if !include(effect.skill_id) {
                continue;
            }
            for (index, slot) in effect.slots.iter().enumerate() {
                let slot_number = index + 1;
                if crate::engine::skill::behavior::registry::find(&slot.behavior).is_none() {
                    tracing::warn!(
                        skill_effect_id = effect.skill_id,
                        slot = slot_number,
                        behavior_opcode = slot.behavior.spec.key.opcode,
                        behavior_type = %slot.behavior.spec.key.type_name,
                        "unregistered behavior in current battle"
                    );
                } else if !crate::engine::skill::behavior::is_supported(&slot.behavior) {
                    tracing::warn!(
                        skill_effect_id = effect.skill_id,
                        slot = slot_number,
                        behavior_opcode = slot.behavior.spec.key.opcode,
                        behavior_type = %slot.behavior.spec.key.type_name,
                        arguments = ?slot.behavior.args,
                        "unsupported behavior arguments in current battle"
                    );
                } else if !crate::engine::skill::behavior::has_destination(&slot.behavior) {
                    tracing::warn!(
                        skill_effect_id = effect.skill_id,
                        slot = slot_number,
                        behavior_opcode = slot.behavior.spec.key.opcode,
                        behavior_type = %slot.behavior.spec.key.type_name,
                        "behavior has no runtime destination in current battle"
                    );
                }

                if let Err(error) = &slot.compiled_route {
                    tracing::warn!(
                        skill_effect_id = effect.skill_id,
                        slot = slot_number,
                        ?error,
                        "unsupported condition route in current battle"
                    );
                }
                warn_unsupported_conditions(effect.skill_id, slot_number, &slot.conditions);
            }
        }
    }

    fn warn_unsupported_buffs(&self, db: &GameDB, include: impl Fn(i32) -> bool) {
        for &buff_id in &self.reachable_buffs {
            if !include(buff_id) {
                continue;
            }
            let Some(buff) = db.skill_buff.get(buff_id) else {
                continue;
            };
            for raw in buff
                .features
                .split('|')
                .map(str::trim)
                .filter(|raw| !raw.is_empty())
            {
                let values = crate::engine::entity::skill::split_ids(raw);
                let Some((&act_id, args)) = values.split_first() else {
                    tracing::warn!(buff_id, raw, "malformed buff act in current battle");
                    continue;
                };
                let Some(act) = db.buff_act.get(act_id) else {
                    if db.skill_buff.get(act_id).is_none() {
                        tracing::warn!(buff_id, act_id, raw, "missing buff act in current battle");
                    }
                    continue;
                };
                let definition =
                    crate::engine::skill::buff_act::registry::find(act.id, &act.r#type);
                if definition.is_none() {
                    tracing::warn!(
                        buff_id,
                        act_id = act.id,
                        act_type = %act.r#type,
                        effect_time = act.effect_time,
                        raw,
                        "unregistered buff act in current battle"
                    );
                } else if crate::engine::skill::buff_act::registry::destination(
                    act.id,
                    &act.r#type,
                    args,
                )
                .is_none()
                {
                    tracing::warn!(
                        buff_id,
                        act_id = act.id,
                        act_type = %act.r#type,
                        effect_time = act.effect_time,
                        raw,
                        "buff act has no semantic consumer in current battle"
                    );
                }
            }
        }
    }
}

fn warn_unsupported_conditions(
    effect_id: i32,
    slot: usize,
    conditions: &[crate::engine::skill::condition::ParsedCondition],
) {
    for condition in conditions {
        match &condition.kind {
            ParsedConditionKind::Unsupported(reason) => tracing::warn!(
                skill_effect_id = effect_id,
                slot,
                condition_opcode = condition.opcode,
                condition_type = %condition.type_name,
                %reason,
                "unsupported condition in current battle"
            ),
            ParsedConditionKind::Any(groups) => {
                for group in groups {
                    warn_unsupported_conditions(effect_id, slot, group);
                }
            }
            ParsedConditionKind::Not(inner) => {
                let nested = crate::engine::skill::condition::ParsedCondition {
                    opcode: condition.opcode,
                    type_name: condition.type_name.clone(),
                    kind: *inner.clone(),
                    raw_args: condition.raw_args.clone(),
                };
                warn_unsupported_conditions(effect_id, slot, std::slice::from_ref(&nested));
            }
            _ => {}
        }
    }
}
