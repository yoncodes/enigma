use super::*;

impl HpManager {
    pub fn sync_entity(&self, entity: &mut FightEntityInfo) {
        let Some(uid) = entity.uid else { return };
        let state = self.get(uid);
        entity.current_hp = Some(state.current);
        if let Some(attr) = entity.attr.as_mut() {
            attr.hp = Some(state.max);
        }
        entity.shield_value = Some(self.shield(uid));
    }

    #[cfg(test)]
    pub(crate) fn execute_command(
        &mut self,
        command: HpCommand,
    ) -> Result<HpChanges, HpCommandError> {
        self.validate_command(command)?;
        Ok(self.commit_validated_command_with_team_shared(command, None))
    }

    pub(crate) fn commit_validated_command_with_team_shared(
        &mut self,
        command: HpCommand,
        team_shared: Option<TeamSharedShieldPlan>,
    ) -> HpChanges {
        debug_assert!(self.validate_command(command).is_ok());
        let (origin, source_uid, target_uid, _) = Self::command_context(command);
        let before = self.current(target_uid);
        let mut changes = HpChanges {
            origin,
            source_uid,
            target_uid,
            damage: None,
            publishes_hit: !matches!(command, HpCommand::SettleAccumulatedDamage(_)),
            fake_hp_consumed: None,
            team_shared_shield_absorbed: None,
            team_shared_shield_removed: None,
            shared_hurt: None,
            shield_absorbed: None,
            shield_granted: None,
            max_hp: None,
            hp: None,
            toughness: None,
            kill: None,
            death: None,
        };
        match command {
            HpCommand::Damage(value) | HpCommand::SettleAccumulatedDamage(value) => {
                changes.damage = Some(DamageRecord {
                    amount: value.amount,
                    config_effect: value.config_effect,
                    effect_kind: value.effect_kind,
                    assassinate: value.assassinate,
                    ignore_riposte: value.ignore_riposte,
                    hurt: value.hurt,
                });
                changes.team_shared_shield_absorbed = team_shared.and_then(|plan| {
                    self.absorb_team_shared_shield(plan, target_uid, value.amount)
                });
                let after_team_shared = value.amount
                    - changes
                        .team_shared_shield_absorbed
                        .map(|change| change.absorbed)
                        .unwrap_or_default();
                changes.shield_absorbed = self.absorb_shield(target_uid, after_team_shared);
                let remaining = value.amount
                    - changes
                        .team_shared_shield_absorbed
                        .map(|change| change.absorbed)
                        .unwrap_or_default()
                    - changes
                        .shield_absorbed
                        .map(|change| change.absorbed)
                        .unwrap_or_default();
                if remaining > 0 {
                    let mut hurt = value.hurt;
                    hurt.from_uid = source_uid;
                    hurt.is_crit = value.effect_kind == DamageEffectKind::Critical;
                    changes.hp =
                        self.lose_with_hurt(target_uid, remaining, value.config_effect, Some(hurt));
                    if let Some(change) = &mut changes.hp {
                        change.assassinate = value.assassinate;
                    }
                }
                if value.hurt.damage_from == HurtDamageFromType::Skill {
                    *self
                        .skill_damage_taken
                        .entry((source_uid, target_uid))
                        .or_default() += changes.applied_damage();
                }
            }
            HpCommand::Lose(value) => {
                if before > 0 {
                    changes.hp = self.lose_with_hurt(
                        target_uid,
                        value.amount,
                        value.config_effect,
                        value.hurt,
                    );
                }
            }
            HpCommand::Kill(value) => {
                changes.hp = self.lose(target_uid, before, value.config_effect);
                changes.kill = Some(value.config_effect);
            }
            HpCommand::Heal(value) => {
                changes.hp = self.heal(target_uid, value.amount, value.config_effect);
                if let Some(change) = &mut changes.hp {
                    change.effect_type = match value.kind {
                        HpHealKind::Normal => 0,
                        HpHealKind::Critical => {
                            sonettobuf::effect_type_enum::EffectType::Healcrit as i32
                        }
                        HpHealKind::Bloodlust => {
                            sonettobuf::effect_type_enum::EffectType::Bloodlust as i32
                        }
                        HpHealKind::Revive => sonettobuf::effect_type_enum::EffectType::Cure as i32,
                    };
                }
            }
            HpCommand::SetCurrent(value) => {
                changes.hp =
                    self.apply_delta(target_uid, value.value - before, value.config_effect, None);
                if let Some(change) = &mut changes.hp {
                    change.effect_type = value.effect_type;
                    change.display_amount = Some(change.after);
                }
            }
            HpCommand::GrantShield(value) => {
                let gain = self.add_shield(target_uid, value.amount, value.max);
                changes.shield_granted = (gain.added != 0).then_some(gain);
            }
            HpCommand::AdjustMax(value) => {
                let before = self.get(target_uid);
                let after = self.add_max_snapshot(target_uid, value.delta);
                if crate::engine::diagnostics::enabled(
                    crate::engine::diagnostics::TraceArea::Damage,
                ) {
                    eprintln!(
                        "max-hp source={} target={target_uid} requested={} before={}/{} after={}/{} origin={origin:?}",
                        source_uid,
                        value.delta,
                        before.current,
                        before.max,
                        after.current,
                        after.max,
                    );
                }
                changes.max_hp = Some(MaxHpChange {
                    target_uid,
                    before_current: before.current,
                    before_max: before.max,
                    delta: after.max - before.max,
                    after_current: after.current,
                    after_max: after.max,
                });
            }
        }
        let after = self.current(target_uid);
        let revision = self.revisions.get(&target_uid).copied().unwrap_or_default();
        if before > 0 && after == 0 {
            changes.death = Some(PendingDeath {
                transition: DeathTransition {
                    source_uid,
                    target_uid,
                },
                revision,
            });
        }

        let applied_damage = i64::from(changes.applied_damage());
        if applied_damage > 0 {
            *self.total_damage_taken.entry(target_uid).or_default() += applied_damage;
            if source_uid != target_uid
                && (changes.damage.is_some() || changes.hp.and_then(|change| change.hurt).is_some())
            {
                *self.total_damage_dealt.entry(source_uid).or_default() += applied_damage;
            }
        }
        if matches!(command, HpCommand::Heal(_))
            && let Some(healed) = changes.hp.filter(|change| change.delta > 0)
        {
            *self.total_healing_done.entry(source_uid).or_default() += i64::from(healed.delta);
        }
        changes
    }

    fn command_context(command: HpCommand) -> (CommandOrigin, i64, i64, bool) {
        match command {
            HpCommand::Damage(value) | HpCommand::SettleAccumulatedDamage(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.amount >= 0,
            ),
            HpCommand::Lose(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.amount > 0 || (value.amount == 0 && value.hurt.is_some()),
            ),
            HpCommand::Kill(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.config_effect > 0,
            ),
            HpCommand::Heal(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.amount > 0,
            ),
            HpCommand::SetCurrent(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.value >= 0 && value.effect_type != 0,
            ),
            HpCommand::GrantShield(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.amount > 0 && value.max > 0,
            ),
            HpCommand::AdjustMax(value) => (
                value.origin,
                value.source_uid,
                value.target_uid,
                value.delta != 0,
            ),
        }
    }

    pub(crate) fn validate_command(&self, command: HpCommand) -> Result<(), HpCommandError> {
        let (_, _, target_uid, valid) = Self::command_context(command);
        if target_uid == 0 || !valid {
            return Err(HpCommandError::InvalidCommand);
        }
        if !self.states.contains_key(&target_uid) {
            return Err(HpCommandError::MissingTarget(target_uid));
        }
        Ok(())
    }

    pub(super) fn apply_delta(
        &mut self,
        uid: i64,
        delta: i32,
        config_effect: i32,
        mut hurt: Option<HurtInfoData>,
    ) -> Option<HpChange> {
        if delta < 0
            && let Some(hurt) = &mut hurt
            && hurt.display_amount.is_none()
        {
            hurt.display_amount = Some(delta.saturating_abs());
        }
        let state = self.states.entry(uid).or_default();
        let before = state.current;
        let after = (before + delta).clamp(0, state.max.max(0));
        let applied = after - before;
        state.current = after;
        if before != after {
            *self.revisions.entry(uid).or_default() += 1;
        }
        if applied < 0 {
            *self.damage_taken.entry(uid).or_default() += -applied;
            if let Some(hurt) = &mut hurt {
                // The client uses damage for the floating number and reduce_hp
                // for the health-bar delta. Keep the latter tied to the amount
                // the HP manager actually committed, including overkill.
                hurt.reduce_hp = applied;
            }
        }

        Some(HpChange {
            target_uid: uid,
            before,
            delta: applied,
            after,
            max: state.max,
            config_effect,
            hurt,
            assassinate: false,
            effect_type: 0,
            display_amount: Some(delta.saturating_abs()),
        })
    }
}
