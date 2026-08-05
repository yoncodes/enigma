use super::*;

/// Reports config reachability and exact support gaps without executing rules.
/// Diagnostics cannot make unsupported configuration runnable.
pub(crate) fn scan_closure(
    db: &config::GameDB,
    catalog: &mut SkillEffectCatalog,
    skills: &mut VecDeque<Pending>,
    buffs: &mut VecDeque<Pending>,
    report: &mut Report,
) {
    while !skills.is_empty() || !buffs.is_empty() {
        while let Some(pending) = skills.pop_front() {
            catalog.extend_roots(db, [pending.id], []);
            scan_skill(db, catalog, pending, skills, buffs, report);
        }
        while let Some(pending) = buffs.pop_front() {
            scan_buff(db, pending, skills, buffs, report);
        }
    }
}

fn scan_skill(
    db: &config::GameDB,
    catalog: &SkillEffectCatalog,
    pending: Pending,
    skills: &mut VecDeque<Pending>,
    buffs: &mut VecDeque<Pending>,
    report: &mut Report,
) {
    if pending.id == 0 || !report.checked_skills.insert(pending.id) {
        return;
    }
    let Some(skill) = db.skill.get(pending.id) else {
        report.error(format!(
            "MissingSkill path={} skill={}",
            pending.path, pending.id
        ));
        return;
    };
    let skill_name = localized(db, &skill.name);
    let skill_label = if skill_name.trim().is_empty() {
        pending.id.to_string()
    } else {
        format!("{} ({skill_name})", pending.id)
    };
    report.explain(format!(
        "Skill id={} effect={} role={} path={}",
        pending.id,
        skill.skill_effect,
        skill_role(catalog, pending.id),
        pending.path
    ));
    for issue in catalog.issues(pending.id) {
        report.error(format!(
            "{:?} path={} > skill {} effect={} slot={} opcode={:?} type={:?} raw={:?}",
            issue.reason,
            pending.path,
            pending.id,
            issue.effect_id,
            issue.slot,
            issue.opcode,
            issue.type_name,
            issue.raw,
        ));
    }
    let Some(effect) = catalog.get(pending.id) else {
        if skill.skill_effect != 0 {
            report.error(format!(
                "MissingSkillEffect path={} > skill {} effect={}",
                pending.path, pending.id, skill.skill_effect
            ));
        }
        return;
    };
    for (index, slot) in effect.slots.iter().enumerate() {
        let slot_path = format!(
            "{} > skill {} > slot {}",
            pending.path,
            skill_label,
            index + 1
        );
        let behavior_key = CapabilityKey::new(
            "behavior",
            slot.behavior.spec.key.opcode,
            &slot.behavior.spec.key.type_name,
        );
        report.capability(behavior_key.clone());
        if !is_supported(&slot.behavior) {
            report.gap(behavior_key.clone(), "unowned behavior");
            report.error(format!(
                "UnownedBehavior path={slot_path} opcode={} type={}",
                slot.behavior.spec.key.opcode, slot.behavior.spec.key.type_name
            ));
        } else if !behavior::has_destination(&slot.behavior) {
            report.gap(behavior_key.clone(), "missing command owner");
            let owner = behavior::registry::find(&slot.behavior)
                .map(|definition| format!("{:?}", definition.output_owner))
                .unwrap_or_else(|| "unowned".to_owned());
            report.error(format!(
                "MissingCommandOwner path={slot_path} opcode={} type={} owner={owner}",
                slot.behavior.spec.key.opcode, slot.behavior.spec.key.type_name,
            ));
        }
        let definition = behavior::registry::find(&slot.behavior);
        match definition.and_then(|definition| definition.supports) {
            None => {
                report.gap(behavior_key.clone(), "unvalidated behavior arguments");
                report.error(format!(
                    "UnvalidatedBehaviorArguments opcode={} type={}",
                    slot.behavior.spec.key.opcode, slot.behavior.spec.key.type_name,
                ));
            }
            Some(supports) if !supports(&slot.behavior) => {
                report.gap(behavior_key, "unsupported behavior arguments");
                report.error(format!(
                    "UnsupportedBehaviorArguments path={slot_path} opcode={} type={} raw={:?}",
                    slot.behavior.spec.key.opcode,
                    slot.behavior.spec.key.type_name,
                    slot.behavior.raw_args,
                ));
            }
            Some(_) => {}
        }
        let references = definition
            .map(|definition| (definition.references)(&slot.behavior))
            .unwrap_or_default();
        report.explain(format!(
            "Slot skill={} slot={} route={} behavior={}:{} kind={:?} phase={:?} destination={} refs=skills{:?}/buffs{:?}",
            pending.id,
            index + 1,
            slot_route(catalog, pending.id, slot),
            slot.behavior.spec.key.opcode,
            slot.behavior.spec.key.type_name,
            slot.behavior.spec.kind,
            definition.map(|definition| definition.phase),
            definition.is_some_and(|definition| definition.destination),
            references.skills,
            references.buffs,
        ));
        match &slot.compiled_route {
            Err(RouteError::ConflictingConditionDrivers { first, second }) => {
                report.error(format!(
                    "ConflictingConditionDrivers path={slot_path} first={first:?} second={second:?}"
                ))
            }
            Err(RouteError::UnregisteredExactKey { opcode, type_name }) => {
                report.gap(
                    CapabilityKey::new("condition", *opcode, type_name),
                    "missing exact route",
                );
                report.error(format!(
                    "MissingConditionRoute path={slot_path} opcode={opcode} type={type_name}"
                ));
            }
            Ok(_) => {}
        }
        explain_conditions(&slot.conditions, pending.id, index + 1, report);
        check_conditions(&slot.conditions, &slot_path, report);
        for target in [&slot.target, &slot.condition_target] {
            let key = CapabilityKey::new("target", target.code, "Target");
            report.capability(key.clone());
            if !is_mapped_target_code(target.code) {
                report.gap(key, "unsupported target");
                report.error(format!(
                    "UnsupportedTarget path={slot_path} code={} raw={:?}",
                    target.code, target.raw
                ));
            }
        }
        enqueue_behavior_references(db, &slot.behavior, &slot_path, skills, buffs, report);
    }
}

fn check_conditions(conditions: &[ParsedCondition], path: &str, report: &mut Report) {
    for condition in conditions {
        let key = CapabilityKey::new("condition", condition.opcode, &condition.type_name);
        report.capability(key.clone());
        match &condition.kind {
            ParsedConditionKind::Unsupported(reason) => {
                report.gap(key, "unsupported condition");
                report.error(format!(
                    "UnsupportedCondition path={path} opcode={} reason={reason} routeHint=unknown; classify the exact key before implementing semantics raw={:?}",
                    condition.opcode, condition.raw_args
                ));
            }
            ParsedConditionKind::Any(groups) => {
                for group in groups {
                    check_conditions(group, path, report);
                }
            }
            ParsedConditionKind::Not(inner) => {
                let nested = ParsedCondition {
                    opcode: condition.opcode,
                    type_name: condition.type_name.clone(),
                    kind: *inner.clone(),
                    raw_args: condition.raw_args.clone(),
                };
                check_conditions(std::slice::from_ref(&nested), path, report);
            }
            _ => {}
        }
    }
}

fn enqueue_behavior_references(
    db: &config::GameDB,
    behavior: &ParsedBehavior,
    path: &str,
    skills: &mut VecDeque<Pending>,
    buffs: &mut VecDeque<Pending>,
    report: &mut Report,
) {
    if let Some(definition) = behavior::registry::find(behavior) {
        let references = (definition.references)(behavior);
        for id in references.skills {
            enqueue(skills, id, path.to_owned());
        }
        for id in references.buffs {
            enqueue(buffs, id, path.to_owned());
        }
        for id in references.models {
            enqueue_monster_skills(db, id, &format!("{path} > model {id}"), skills, report);
        }
    }
}

pub(super) fn enqueue_monster_skills(
    db: &config::GameDB,
    monster_id: i32,
    path: &str,
    skills: &mut VecDeque<Pending>,
    report: &mut Report,
) {
    let Some(monster) = db.monster.get(monster_id) else {
        report.error(format!("MissingMonster path={path}"));
        return;
    };
    let Some(template) = db.monster_skill_template.get(monster.skill_template) else {
        report.error(format!(
            "MissingMonsterSkillTemplate path={path} template={}",
            monster.skill_template
        ));
        return;
    };
    for skill_id in parse_skill_group(&template.active_skill, 1)
        .into_iter()
        .chain(parse_skill_group(&template.active_skill, 2))
        .chain(split_ids(&template.passive_skill))
        .chain(split_ids(&monster.passive_skills_ex))
        .chain(split_ids(&template.unique_skill))
        .chain(db.toughness_passive_skills(monster.toughness_skill))
    {
        enqueue(skills, skill_id, path.to_owned());
    }
}

fn scan_buff(
    db: &config::GameDB,
    pending: Pending,
    skills: &mut VecDeque<Pending>,
    buffs: &mut VecDeque<Pending>,
    report: &mut Report,
) {
    if pending.id == 0 || !report.checked_buffs.insert(pending.id) {
        return;
    }
    let Some(buff) = db.skill_buff.get(pending.id) else {
        report.error(format!(
            "MissingBuff path={} buff={}",
            pending.path, pending.id
        ));
        return;
    };
    report.explain(format!(
        "Buff id={} type={} path={}",
        pending.id, buff.type_id, pending.path
    ));
    let handler_owns_duration = buff.features.split('|').any(|raw| {
        let Some(act_id) = split_ids(raw).first().copied() else {
            return false;
        };
        db.buff_act
            .get(act_id)
            .is_some_and(|act| buff_act_registry::owns_duration(act.id, &act.r#type))
    });
    match BuffPolicy::try_for_buff_id(pending.id) {
        Ok(policy) => {
            report.explain(format!(
                "BuffPolicy buff={} storage={:?} match={:?} duplicate={:?} duration={} takeStage={} uid={:?} exclusions={:?} unresolvedIncludeEntries={:?}",
                pending.id,
                policy.storage,
                policy.match_existing,
                policy.on_duplicate,
                policy.lifetime.duration,
                policy.lifetime.take_stage,
                policy.uid,
                policy.exclusions.remove_on_grant,
                policy.unresolved_include_entries,
            ));
            if !policy.unresolved_include_entries.is_empty() {
                let buff_name = localized(db, &buff.name);
                let buff_label = if buff_name.trim().is_empty() {
                    pending.id.to_string()
                } else {
                    format!("{} ({buff_name})", pending.id)
                };
                for &(include_type, value) in policy.unresolved_include_entries.iter() {
                    report.gap_at(
                        CapabilityKey::new(
                            "buff-include",
                            include_type,
                            BuffPolicy::include_entry_name(include_type, value),
                        ),
                        "UnresolvedIncludePolicy",
                        format!("{} > buff {buff_label}", pending.path),
                    );
                }
                report.error(format!(
                    "UnresolvedBuffIncludeEntries path={} buff={} includeEntries={}",
                    pending.path,
                    pending.id,
                    policy
                        .unresolved_include_entries
                        .iter()
                        .map(|&(include_type, value)| BuffPolicy::include_entry_name(
                            include_type,
                            value
                        ))
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            if policy.lifetime.duration > 0
                && !handler_owns_duration
                && !battle::engine::skill::buff_act::effect_time::has_duration_advance_route(
                    policy.lifetime.take_stage,
                )
            {
                report.gap_at(
                    CapabilityKey::new("effect-time", policy.lifetime.take_stage, "BuffDuration"),
                    "MissingBuffDurationRoute",
                    format!("{} > buff {}", pending.path, pending.id),
                );
                report.error(format!(
                    "MissingBuffDurationRoute path={} buff={} duration={} takeStage={}",
                    pending.path, pending.id, policy.lifetime.duration, policy.lifetime.take_stage,
                ));
            }
        }
        Err(error) => report.error(format!(
            "InvalidBuffPolicy path={} buff={} reason={error:?}",
            pending.path, pending.id
        )),
    }
    for linked_buff_id in halo::carriers(pending.id)
        .into_iter()
        .filter_map(|carrier| carrier.linked_buff_id)
    {
        enqueue(
            buffs,
            linked_buff_id,
            format!("{} > buff {}", pending.path, pending.id),
        );
    }
    for raw in buff
        .features
        .split('|')
        .filter(|raw| !raw.trim().is_empty())
    {
        let values = split_ids(raw);
        let Some(&feature_id) = values.first() else {
            report.error(format!(
                "MalformedBuffFeature path={} > buff {} raw={raw:?}",
                pending.path, pending.id
            ));
            continue;
        };
        if let Some(act) = db.buff_act.get(feature_id) {
            let key = CapabilityKey::new("buff-act", act.id, &act.r#type);
            report.capability(key.clone());
            let definition = buff_act_registry::find(act.id, &act.r#type);
            let route = classify_effect_time(act.effect_time);
            let runtime_event =
                buff_act::registry::runtime_event(act.id, &act.r#type, act.effect_time);
            let has_runtime_route = runtime_event.is_some()
                || definition.is_some_and(|definition| {
                    !definition.runtime.events.is_empty()
                        || !definition.transaction.events.is_empty()
                });
            let destination = buff_act::registry::destination(act.id, &act.r#type, &values[1..]);
            let has_destination = destination.is_some();
            let wire = buff_act::wire::find(act.id, &act.r#type);
            if let Some(wire) = wire {
                for phase in [
                    buff_act::wire::WirePhase::Add,
                    buff_act::wire::WirePhase::Static,
                    buff_act::wire::WirePhase::Refresh,
                ] {
                    for &effect_type in wire.markers(phase) {
                        let evidence =
                            report
                                .wire_evidence
                                .source(act.id, &act.r#type, phase, effect_type);
                        report.explain(format!(
                            "BuffActWire act={} type={} phase={phase:?} effectType={effect_type} evidence={}",
                            act.id, act.r#type,
                            evidence
                                .as_deref()
                                .map_or("Inferred".to_owned(), |source| format!("Captured({source})")),
                        ));
                        if evidence.is_none() {
                            report.warning(format!(
                                "InferredBuffActMarker act={} type={} phase={phase:?} effectType={effect_type}",
                                act.id, act.r#type,
                            ));
                        }
                    }
                }
            }
            let capability = buff_act_capability(destination);
            report.explain(format!(
                "BuffAct buff={} act={} type={} handler={} capability={} statRead={:?} event={:?} events={:?} transactions={:?} setup={:?} route={:?} effectTime={}",
                pending.id,
                act.id,
                act.r#type,
                definition
                    .map(|definition| format!("{:?}", definition.kind))
                    .unwrap_or_else(|| "unregistered".to_owned()),
                capability.unwrap_or("missing"),
                definition.map(|definition| definition.state.read_timing),
                runtime_event,
                definition.map(|definition| definition.runtime.events),
                definition.map(|definition| definition.transaction.events),
                definition.map(|definition| definition.setup.routes),
                route,
                act.effect_time,
            ));
            if definition.is_none() {
                report.gap(key.clone(), "unregistered buff act");
                let message = format!(
                    "UnregisteredBuffAct path={} > buff {} act={} type={} route={route:?} effectTime={}",
                    pending.path, pending.id, act.id, act.r#type, act.effect_time
                );
                if route == BuffActEvent::StaticRead {
                    report.warning(message);
                } else {
                    report.error(message);
                }
            }
            if has_runtime_route && !has_destination {
                report.gap(key.clone(), "missing destination");
                report.error(format!(
                    "MissingDestinationBuffAct path={} > buff {} act={} type={} route={route:?} effectTime={}",
                    pending.path, pending.id, act.id, act.r#type, act.effect_time
                ));
            } else if definition.is_some() && capability.is_none() {
                report.gap(key.clone(), "missing semantic consumer");
                report.error(format!(
                    "MissingSemanticConsumer path={} > buff {} act={} type={} route={route:?} effectTime={}",
                    pending.path, pending.id, act.id, act.r#type, act.effect_time
                ));
            }
            if matches!(route, BuffActEvent::Runtime(_))
                && definition.is_some_and(|definition| definition.runtime.effect_time_subscription)
                && runtime_event.is_none()
            {
                report.gap(key.clone(), "missing subscriber");
                report.error(format!(
                    "MissingSubscriber path={} > buff {} act={} type={} route={route:?} effectTime={}",
                    pending.path, pending.id, act.id, act.r#type, act.effect_time
                ));
            }
            if definition.is_some() && wire.is_none() {
                report.gap(key.clone(), "missing wire rule");
                report.warning(format!(
                    "MissingWireRule path={} > buff {} act={} type={} route={route:?} effectTime={}",
                    pending.path, pending.id, act.id, act.r#type, act.effect_time
                ));
            }
            if matches!(route, BuffActEvent::Unknown(_)) {
                report.gap(key, "unknown effect time");
                report.error(format!(
                    "UnknownEffectTime path={} > buff {} act={} type={} effectTime={}",
                    pending.path, pending.id, act.id, act.r#type, act.effect_time
                ));
            }
            match definition.map(|definition| definition.kind) {
                Some(BuffActKind::SubBuff) => {
                    if let Some(&buff_id) = values.get(1) {
                        enqueue(
                            buffs,
                            buff_id,
                            format!("{} > buff {}", pending.path, pending.id),
                        );
                    }
                }
                Some(BuffActKind::AddBuffToEnter) => {
                    if let Some(buff_id) =
                        buff_act::add_buff_to_enter::referenced_buff(&values[1..])
                    {
                        enqueue(
                            buffs,
                            buff_id,
                            format!("{} > buff {}", pending.path, pending.id),
                        );
                    }
                }
                Some(BuffActKind::TransferEnergyBuff) => {
                    if let Some(buff_id) =
                        buff_act::transfer_energy_buff::referenced_buff(&values[1..])
                    {
                        enqueue(
                            buffs,
                            buff_id,
                            format!("{} > buff {}", pending.path, pending.id),
                        );
                    }
                }
                Some(BuffActKind::AddPassiveSkills) => {
                    if let Some(&skill_id) = values.get(1) {
                        enqueue(
                            skills,
                            skill_id,
                            format!("{} > buff {}", pending.path, pending.id),
                        );
                    }
                }
                Some(BuffActKind::AddSpTempCard) => {
                    if let Some(&skill_id) = values.get(1) {
                        enqueue(
                            skills,
                            skill_id,
                            format!("{} > buff {}", pending.path, pending.id),
                        );
                    }
                }
                Some(BuffActKind::BeatBack) => {
                    if let Some(skill_id) = buff_act::riposte::holder_skill(&values[1..]) {
                        enqueue(
                            skills,
                            skill_id,
                            format!("{} > buff {}", pending.path, pending.id),
                        );
                    }
                }
                Some(BuffActKind::CardNotCalSize) => {
                    for &skill_id in values
                        .iter()
                        .skip(1)
                        .filter(|id| db.skill.get(**id).is_some())
                    {
                        enqueue(
                            skills,
                            skill_id,
                            format!("{} > buff {}", pending.path, pending.id),
                        );
                    }
                }
                Some(BuffActKind::AdrenalineAddCard) => {
                    if let Some(raw_skills) = raw.split('#').nth(2) {
                        for skill_id in raw_skills
                            .split(',')
                            .filter_map(|value| value.trim().parse::<i32>().ok())
                        {
                            enqueue(
                                skills,
                                skill_id,
                                format!("{} > buff {}", pending.path, pending.id),
                            );
                        }
                    }
                }
                Some(BuffActKind::NuoDiKaCastChannel) => {
                    for skill_id in
                        buff_act::nuo_di_ka_cast_channel::referenced_skills(&values[1..])
                    {
                        enqueue(
                            skills,
                            skill_id,
                            format!("{} > buff {}", pending.path, pending.id),
                        );
                    }
                }
                Some(BuffActKind::SpecialCountCastChannel) => {
                    if let Some(&skill_id) = values.get(1) {
                        enqueue(
                            skills,
                            skill_id,
                            format!("{} > buff {}", pending.path, pending.id),
                        );
                    }
                }
                Some(BuffActKind::CastChannel) => {
                    if let Some(skill_id) = buff_act::cast_channel::referenced_skill(&values[1..]) {
                        enqueue(
                            skills,
                            skill_id,
                            format!("{} > buff {}", pending.path, pending.id),
                        );
                    }
                }
                Some(BuffActKind::AddCardCastChannel) => {
                    if let Some(skill_id) =
                        buff_act::add_card_cast_channel::referenced_skill(&values[1..])
                    {
                        enqueue(
                            skills,
                            skill_id,
                            format!("{} > buff {}", pending.path, pending.id),
                        );
                    }
                }
                Some(BuffActKind::BeAttackedAssassinate) if raw.split('#').nth(3).is_some() => {
                    report.warning(format!(
                        "PartialBuffAct path={} > buff {} act={} type={} unsupported=skill-buff-map raw={raw:?}",
                        pending.path, pending.id, act.id, act.r#type
                    ));
                }
                Some(BuffActKind::BeatBackDependOnAttackMe) => {
                    for &skill_id in values.iter().skip(1).take(2) {
                        enqueue(
                            skills,
                            skill_id,
                            format!("{} > buff {}", pending.path, pending.id),
                        );
                    }
                }
                _ => {}
            }
        } else if db.skill_buff.get(feature_id).is_some() {
            enqueue(
                buffs,
                feature_id,
                format!("{} > buff {}", pending.path, pending.id),
            );
        } else {
            report.error(format!(
                "MissingBuffAct path={} > buff {} feature={} raw={raw:?}",
                pending.path, pending.id, feature_id
            ));
        }
    }
}

pub(super) fn buff_act_capability(
    destination: Option<battle::engine::skill::buff_act::registry::BuffActDestination>,
) -> Option<&'static str> {
    use battle::engine::skill::buff_act::registry::BuffActDestination;

    destination.map(|destination| match destination {
        BuffActDestination::Runtime => "subscriber",
        BuffActDestination::Transaction => "transaction",
        BuffActDestination::Setup => "setup",
        BuffActDestination::AttackReplacement => "damage-formula",
        BuffActDestination::StateConsumer => "state-consumer",
        BuffActDestination::LinkedSkill => "linked-skill",
    })
}

fn skill_role(catalog: &SkillEffectCatalog, skill_id: i32) -> &'static str {
    if catalog.is_passive(skill_id) {
        "passive"
    } else if catalog.is_big_skill(skill_id) {
        "ultimate"
    } else {
        "active"
    }
}

fn slot_route(catalog: &SkillEffectCatalog, skill_id: i32, slot: &SkillEffectSlot) -> String {
    if let Ok(route) = &slot.compiled_route
        && let Some(label) = compiled_route_label(route)
    {
        return label;
    }

    let conditions = &slot.conditions;
    let timings = conditions
        .iter()
        .flat_map(flatten_conditions)
        .filter_map(|condition| match condition.timing() {
            ConditionTiming::Event(event) => Some(format!("{event:?}")),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    if !timings.is_empty() {
        return format!(
            "event:{}",
            timings.into_iter().collect::<Vec<_>>().join("+")
        );
    }
    if !catalog.is_passive(skill_id) {
        return format!("{}-direct", skill_role(catalog, skill_id));
    }
    "static-passive".to_owned()
}

fn compiled_route_label(route: &ConditionRoute) -> Option<String> {
    let branches = route
        .branches
        .iter()
        .filter_map(|branch| branch.driver)
        .map(|driver| match driver {
            ConditionDriver::Trigger(trigger) => match trigger.phase {
                Some(phase) => format!("event:{:?}/{phase:?}", trigger.event),
                None => format!("event:{:?}", trigger.event),
            },
            ConditionDriver::Setup(setup) => {
                format!("setup:{:?}/{}", setup.stage, setup.priority)
            }
        })
        .collect::<Vec<_>>();

    (!branches.is_empty()).then(|| branches.join("|"))
}

fn explain_conditions(
    conditions: &[ParsedCondition],
    skill_id: i32,
    slot: usize,
    report: &mut Report,
) {
    for condition in conditions.iter().flat_map(flatten_conditions) {
        report.explain(format!(
            "Condition skill={skill_id} slot={slot} opcode={} type={} role={:?} dependencies={:?} timing={:?} kind={:?}",
            condition.opcode,
            condition.type_name,
            registry::find_key(condition.opcode, &condition.type_name)
                .map(|definition| definition.role),
            registry::find_key(condition.opcode, &condition.type_name)
                .map(|definition| definition.dependencies),
            condition.timing(),
            condition.kind,
        ));
    }
}

fn flatten_conditions(condition: &ParsedCondition) -> Vec<&ParsedCondition> {
    match &condition.kind {
        ParsedConditionKind::Any(groups) => groups
            .iter()
            .flatten()
            .flat_map(flatten_conditions)
            .collect(),
        ParsedConditionKind::Not(_) => vec![condition],
        _ => vec![condition],
    }
}

pub(super) fn configured_skill_ids(raw: &str, db: &config::GameDB) -> Vec<i32> {
    numeric_ids(raw)
        .into_iter()
        .filter(|id| db.skill.get(*id).is_some())
        .collect()
}

fn localized<'a>(db: &'a config::GameDB, value: &'a str) -> &'a str {
    db.language_en.get(value).unwrap_or(value)
}

fn numeric_ids(raw: &str) -> Vec<i32> {
    raw.split(|character: char| !character.is_ascii_digit() && character != '-')
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse().ok())
        .filter(|id| *id > 0)
        .collect()
}

pub(super) fn apply_destiny(
    skills: &mut [i32],
    destiny: Option<&std::collections::HashMap<i32, i32>>,
) {
    let Some(destiny) = destiny else { return };
    for skill in skills {
        if let Some(replacement) = destiny.get(skill) {
            *skill = *replacement;
        }
    }
}

pub(super) fn enqueue(queue: &mut VecDeque<Pending>, id: i32, path: String) {
    if id > 0 {
        queue.push_back(Pending { id, path });
    }
}
