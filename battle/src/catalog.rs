use crate::engine::manager::field::{FieldDefinition, FieldThreshold};
use crate::engine::mechanic::impromptu::ImpromptuDefinition;
use crate::engine::round::power::ClothPower;
use crate::engine::skill::rule::{CommandOrigin, RuleDomain};

const BURN_BUFF_FIGHT_CONST: i32 = 29;
const CONTRACT_OWNER_BUFF_MAP: i32 = 30;
const CONTRACT_BOUND_BUFF_MAP: i32 = 31;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MagicCircleDefinition {
    pub duration: i32,
    pub allied_attributes: Vec<(i32, i32)>,
    pub enemy_attributes: Vec<(i32, i32)>,
    pub allied_buffs: Vec<i32>,
    pub enemy_buffs: Vec<i32>,
    pub self_skills: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfiguredBuffFeature {
    pub act_type: String,
    pub effect_time: i32,
    pub effect_condition: i32,
    pub raw: String,
    pub values: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ConfiguredPlayerSkill {
    pub skill_id: i32,
    pub need_power: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConfiguredTeachingCards {
    pub opening_cards: String,
    pub refill_cards: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EntityExAttributes {
    pub crit_rate: i32,
    pub crit_resist: i32,
    pub crit_dmg: i32,
    pub crit_def: i32,
    pub add_dmg: i32,
    pub drop_dmg: i32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MonsterResistances {
    pub dizzy: i32,
    pub sleep: i32,
    pub petrified: i32,
    pub frozen: i32,
    pub disarm: i32,
    pub forbid: i32,
    pub seal: i32,
    pub cant_get_exskill: i32,
    pub del_ex_point: i32,
    pub stress_up: i32,
    pub control_resilience: i32,
    pub del_ex_point_resilience: i32,
    pub stress_up_resilience: i32,
    pub charm: i32,
}

impl Default for EntityExAttributes {
    fn default() -> Self {
        Self {
            crit_rate: 0,
            crit_resist: 0,
            crit_dmg: 1000,
            crit_def: 0,
            add_dmg: 0,
            drop_dmg: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LingeringGlowAttributeBuff {
    pub buff_id: i32,
    pub origin: CommandOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfiguredFightVersion {
    Missing,
    Invalid,
    Value(i32),
}

#[derive(Clone, Copy)]
pub struct BattleCatalog {
    game_data: &'static config::GameDB,
    fight_version: ConfiguredFightVersion,
    impromptu_definition: Option<ImpromptuDefinition>,
    lingering_glow_attribute_buff: Option<LingeringGlowAttributeBuff>,
}

impl PartialEq for BattleCatalog {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.game_data, other.game_data)
    }
}

impl Eq for BattleCatalog {}

impl BattleCatalog {
    pub fn new(game_data: &'static config::GameDB) -> Self {
        Self {
            game_data,
            fight_version: configured_fight_version(
                game_data.r#const.get(1707).map(|row| row.value.as_str()),
            ),
            impromptu_definition: impromptu_definition(game_data),
            lingering_glow_attribute_buff: lingering_glow_attribute_buff(game_data),
        }
    }

    pub(crate) fn game_data(self) -> &'static config::GameDB {
        self.game_data
    }

    pub(crate) fn fight_version(self) -> ConfiguredFightVersion {
        self.fight_version
    }

    pub(crate) fn lingering_glow_attribute_buff(self) -> Option<LingeringGlowAttributeBuff> {
        self.lingering_glow_attribute_buff
    }

    pub(crate) fn impromptu_definition(self) -> Option<ImpromptuDefinition> {
        self.impromptu_definition
    }

    pub(crate) fn magic_circle(self, circle_id: i32) -> Option<MagicCircleDefinition> {
        magic_circle_definition(self.game_data, circle_id)
    }

    pub(crate) fn magic_circle_thresholds(self) -> Vec<FieldThreshold> {
        self.game_data
            .fight_dnsz
            .iter()
            .filter_map(|threshold| {
                let circle = self.game_data.magic_circle.get(threshold.id)?;
                Some(FieldThreshold {
                    level: threshold.level,
                    progress: threshold.progress,
                    definition: FieldDefinition {
                        field_id: threshold.id,
                        duration: circle.round,
                    },
                })
            })
            .collect()
    }

    pub(crate) fn buff_has_effect_count(self, buff_id: i32) -> bool {
        self.game_data
            .skill_buff
            .get(buff_id)
            .is_some_and(|buff| buff.effect_count > 0)
    }

    pub(crate) fn buff_expires_after_owner_attack(self, buff_id: i32) -> bool {
        let Some(buff) = self.game_data.skill_buff.get(buff_id) else {
            return false;
        };
        let type_id = if buff.type_id == 0 {
            buff.id
        } else {
            buff.type_id
        };
        self.game_data
            .skill_bufftype
            .get(type_id)
            .is_some_and(|buff_type| buff_type.take_act == "1")
    }

    pub(crate) fn buff_features(self, buff_id: i32) -> Vec<ConfiguredBuffFeature> {
        self.game_data
            .skill_buff
            .get(buff_id)
            .into_iter()
            .flat_map(|buff| buff.features.split('|'))
            .filter_map(|raw| {
                let values = raw
                    .split('#')
                    .map(str::parse)
                    .collect::<Result<Vec<i32>, _>>()
                    .ok()?;
                let act = self.game_data.buff_act.get(*values.first()?)?;
                Some(ConfiguredBuffFeature {
                    act_type: act.r#type.clone(),
                    effect_time: act.effect_time,
                    effect_condition: act.effect_condition,
                    raw: raw.to_owned(),
                    values,
                })
            })
            .collect()
    }

    pub(crate) fn buff_feature_tokens(self, buff_id: i32) -> Vec<String> {
        self.game_data
            .skill_buff
            .get(buff_id)
            .map(|row| row.features.as_str())
            .unwrap_or_default()
            .split('|')
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(str::to_owned)
            .collect()
    }

    pub(crate) fn buff_feature_rows(self, buff_id: i32) -> Vec<&'static str> {
        self.game_data
            .skill_buff
            .get(buff_id)
            .into_iter()
            .flat_map(|buff| buff.features.split('|'))
            .collect()
    }

    pub(crate) fn buff_has_master_halo(self, buff_id: i32) -> bool {
        self.buff_feature_rows(buff_id).into_iter().any(|feature| {
            matches!(
                feature
                    .split('#')
                    .next()
                    .and_then(|value| value.parse().ok()),
                Some(771 | 772 | 822)
            )
        })
    }

    pub(crate) fn buff_pool(self, buff_id: i32) -> Option<Vec<i32>> {
        self.game_data.skill_buff.get(buff_id).map(|row| {
            row.features
                .split('#')
                .filter_map(|entry| entry.split(',').next()?.trim().parse().ok())
                .filter(|buff_id| *buff_id > 0)
                .collect()
        })
    }

    pub(crate) fn buff_type_id(self, buff_id: i32) -> i32 {
        self.game_data
            .skill_buff
            .get(buff_id)
            .map(|row| row.type_id)
            .unwrap_or_default()
    }

    pub(crate) fn burn_buff_type_id(self) -> Option<i32> {
        self.game_data
            .fight_const
            .get(BURN_BUFF_FIGHT_CONST)?
            .value
            .parse()
            .ok()
    }

    pub(crate) fn target_count(self, code: i32) -> i32 {
        target_count(self.game_data, code)
    }

    pub(crate) fn damage_target_count_kind(self, code: i32) -> i32 {
        damage_target_count_kind(self.game_data, code)
    }

    pub(crate) fn contract_binding_buffs(
        self,
        ex_skill_level: i32,
        career: i32,
    ) -> Option<(i32, i32)> {
        contract_binding_buffs(self.game_data, ex_skill_level, career)
    }

    pub(crate) fn summoned_unique_skills(self, summoned_id: i32) -> Vec<i32> {
        summoned_unique_skills(self.game_data, summoned_id)
    }

    pub(crate) fn monster_toughness(self, model_id: i32, max_hp: i32) -> Option<(i32, i32)> {
        configured_monster_toughness(self.game_data, model_id, max_hp)
    }

    pub(crate) fn entity_ex_point_max(
        self,
        explicit_max: Option<i32>,
        model_id: Option<i32>,
        level: i32,
    ) -> Option<i32> {
        configured_ex_point_max(self.game_data, explicit_max, model_id, level)
    }

    pub(crate) fn upgrade_selection(
        self,
        upgrade_id: i32,
        option_id: i32,
    ) -> Option<crate::engine::manager::upgrade::UpgradeSelection> {
        let upgrade = self.game_data.hero_upgrade.get(upgrade_id)?;
        parse_upgrade_ids(&upgrade.options)
            .contains(&option_id)
            .then_some(())?;
        let option = self.game_data.hero_upgrade_options.get(option_id)?;
        Some(crate::engine::manager::upgrade::UpgradeSelection {
            upgrade_id,
            option_id,
            add_buff_ids: parse_upgrade_ids(&option.add_buff),
            del_buff_ids: parse_upgrade_ids(&option.del_buff),
            replace_skill_group1: parse_upgrade_ids(&option.replace_skill_group1),
            replace_skill_group2: parse_upgrade_ids(&option.replace_skill_group2),
            replace_big_skill: option.replace_big_skill,
            replace_passive_skills: parse_upgrade_pairs(&option.replace_passive_skill),
            add_passive_skill_ids: parse_upgrade_ids(&option.add_passive_skill),
        })
    }

    pub(crate) fn upgrade_has_available_option(
        self,
        upgrade_id: i32,
        selected: &[i32],
    ) -> Option<bool> {
        let upgrade = self.game_data.hero_upgrade.get(upgrade_id)?;
        Some(
            parse_upgrade_ids(&upgrade.options)
                .into_iter()
                .any(|option_id| !selected.contains(&option_id)),
        )
    }

    pub(crate) fn buff_status(
        self,
        buff_id: i32,
    ) -> Option<crate::engine::manager::buff::BuffStatus> {
        let buff = self.game_data.skill_buff.get(buff_id)?;
        let type_id = if buff.type_id == 0 {
            buff.id
        } else {
            buff.type_id
        };
        let status_id = self
            .game_data
            .skill_bufftype
            .get(type_id)
            .map(|buff_type| buff_type.r#type)
            .unwrap_or(buff.is_good_buff);
        Some(crate::engine::manager::buff::BuffStatus::from_id(status_id))
    }

    pub(crate) fn buff_act_definition(
        self,
        opcode: i32,
    ) -> Option<&'static crate::engine::skill::buff_act::registry::BuffActDefinition> {
        let act = self.game_data.buff_act.get(opcode)?;
        crate::engine::skill::buff_act::registry::find(opcode, &act.r#type)
    }

    pub(crate) fn buff_act_origin(
        self,
        opcode: i32,
        expected_kind: crate::engine::skill::buff_act::registry::BuffActKind,
    ) -> Option<CommandOrigin> {
        let definition = self.buff_act_definition(opcode)?;
        (definition.kind == expected_kind).then_some(CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: definition.key,
        })
    }

    pub(crate) fn skill_effect_id(self, skill_id: i32) -> i32 {
        self.game_data
            .skill
            .get(skill_id)
            .map(|skill| skill.skill_effect)
            .filter(|effect_id| *effect_id != 0)
            .unwrap_or(skill_id)
    }

    pub(crate) fn skill_hero_id(self, skill_id: i32) -> Option<i32> {
        self.game_data
            .skill
            .get(skill_id)
            .map(|skill| skill.hero_id)
    }

    pub(crate) fn skill_big_skill_point(self, skill_id: i32) -> i32 {
        self.skill_effect(skill_id)
            .map(|effect| effect.big_skill_point)
            .unwrap_or_default()
    }

    pub(crate) fn skill_is_big(self, skill_id: i32) -> bool {
        self.skill_effect(skill_id)
            .is_some_and(|effect| effect.is_big_skill != 0)
    }

    pub(crate) fn skill_effect_tag(self, skill_id: i32) -> i32 {
        self.skill_effect(skill_id)
            .map(|effect| effect.effect_tag)
            .unwrap_or_default()
    }

    pub(crate) fn skill_extra_kind(self, skill_id: i32) -> i32 {
        self.skill_effect(skill_id)
            .map(|effect| effect.is_extra)
            .unwrap_or_default()
    }

    pub(crate) fn skill_type(self, skill_id: i32) -> i32 {
        self.skill_effect(skill_id)
            .map(|effect| effect.r#type)
            .unwrap_or_default()
    }

    pub(crate) fn skill_is_attack(self, skill_id: i32) -> bool {
        self.skill_effect(skill_id).is_some_and(|effect| {
            effect.damage_rate > 0
                || matches!(
                    effect.effect_tag,
                    tag if tag
                        == crate::engine::skill::effect::catalog::SkillEffectTag::RealityDamage
                            as i32
                        || tag
                            == crate::engine::skill::effect::catalog::SkillEffectTag::MentalDamage
                                as i32
                )
        })
    }

    pub(crate) fn player_skills(self, cloth_id: Option<i32>) -> Vec<ConfiguredPlayerSkill> {
        let Some(cloth) = self.cloth(cloth_id) else {
            return Vec::new();
        };

        [
            ConfiguredPlayerSkill {
                skill_id: cloth.skill1,
                need_power: Some(cloth.use_power1.first().copied().unwrap_or(0)),
            },
            ConfiguredPlayerSkill {
                skill_id: cloth.skill2,
                need_power: Some(cloth.use_power2.first().copied().unwrap_or(0)),
            },
            ConfiguredPlayerSkill {
                skill_id: cloth.skill3,
                need_power: None,
            },
        ]
        .into_iter()
        .filter(|skill| skill.skill_id != 0)
        .collect()
    }

    pub(crate) fn cloth_power(self, fight: &sonettobuf::Fight) -> Option<ClothPower> {
        let cloth = self.cloth(fight.attacker.as_ref()?.cloth_id)?;
        Some(ClothPower::configured(
            cloth.max_power,
            cloth.r#use,
            cloth.r#move,
            cloth.compose,
            &cloth.recover,
        ))
    }

    pub(crate) fn cloth_skill_terms(
        self,
        cloth_id: Option<i32>,
        skill_id: i32,
        use_count: usize,
    ) -> Option<(i32, i32, i32)> {
        let cloth = self.cloth(cloth_id)?;
        let (costs, cooldown) = if cloth.skill1 == skill_id {
            (&cloth.use_power1, cloth.cd1)
        } else if cloth.skill2 == skill_id {
            (&cloth.use_power2, cloth.cd2)
        } else {
            return None;
        };
        let cost = *costs.get(use_count.min(costs.len().checked_sub(1)?))?;
        let next_cost = *costs
            .get((use_count + 1).min(costs.len().saturating_sub(1)))
            .unwrap_or(&cost);
        Some((cost, next_cost, cooldown))
    }

    pub(crate) fn card_enchant_excluded_ids(self, enchant_id: i32) -> Vec<i32> {
        self.card_enchant_ids(enchant_id, |row| &row.exclude_types)
    }

    pub(crate) fn card_enchant_rejected_ids(self, enchant_id: i32) -> Vec<i32> {
        self.card_enchant_ids(enchant_id, |row| &row.reject_types)
    }

    pub(crate) fn card_enchant_current_hp_loss_permille(self, enchant_id: i32) -> Option<i32> {
        card_enchant_current_hp_loss_permille(self.game_data, enchant_id)
    }

    pub(crate) fn skill_rank(self, skill_id: i32) -> i32 {
        self.game_data
            .skill
            .get(skill_id)
            .map(|row| row.skill_rank)
            .unwrap_or_default()
    }

    pub(crate) fn card_skill_rank(self, card: &sonettobuf::CardInfo) -> i32 {
        card.skill_id
            .and_then(|skill_id| self.game_data.skill.get(skill_id))
            .map(|row| row.skill_rank)
            .unwrap_or_else(|| card.card_effect.unwrap_or_default())
    }

    pub(crate) fn skill_is_ultimate_for_model(self, skill_id: i32, model_id: i32) -> bool {
        self.game_data
            .skill
            .get(skill_id)
            .is_some_and(|skill| skill.hero_id == model_id && self.skill_is_big(skill_id))
    }

    pub(crate) fn fight_const_value(self, id: i32) -> i32 {
        self.game_data
            .fight_const
            .get(id)
            .and_then(|row| row.value.parse().ok())
            .unwrap_or_default()
    }

    pub(crate) fn career_multiplier(self, source: i32, target: i32) -> i32 {
        let Some(row) = self.game_data.fight_effect.get(source) else {
            return 1000;
        };
        match target {
            1 => row.career1,
            2 => row.career2,
            3 => row.career3,
            4 => row.career4,
            5 => row.career5,
            6 => row.career6,
            7 => row.career7,
            8 => row.career8,
            _ => 1000,
        }
    }

    pub(crate) fn strongest_career_multiplier(self, source: i32) -> i32 {
        let Some(row) = self.game_data.fight_effect.get(source) else {
            return 1000;
        };
        [
            row.career1,
            row.career2,
            row.career3,
            row.career4,
            row.career5,
            row.career6,
            row.career7,
            row.career8,
        ]
        .into_iter()
        .max()
        .unwrap_or(1000)
    }

    pub(crate) fn boss_model_ids(self, fight: &sonettobuf::Fight) -> Vec<i32> {
        let Some(battle) = self.configured_battle(fight) else {
            return Vec::new();
        };
        let wave = fight.cur_wave.unwrap_or(1).max(1) as usize - 1;
        battle
            .monster_group_ids
            .split('#')
            .filter_map(|id| id.parse::<i32>().ok())
            .nth(wave)
            .and_then(|group_id| self.game_data.monster_group.get(group_id))
            .into_iter()
            .flat_map(|group| group.boss_id.split('#'))
            .filter_map(|id| id.parse().ok())
            .collect()
    }

    pub(crate) fn teaching_cards(self, episode_id: i32) -> Option<ConfiguredTeachingCards> {
        configured_teaching_cards(self.game_data, episode_id)
    }

    pub(crate) fn device_card_weights(self, model_id: i32) -> Vec<(i32, usize)> {
        configured_device_card_weights(self.game_data, model_id)
    }

    pub(crate) fn skill_effects_for_fight(
        self,
        fight: &sonettobuf::Fight,
    ) -> crate::engine::skill::effect::SkillEffectCatalog {
        crate::engine::skill::effect::SkillEffectCatalog::from_fight(self.game_data, fight)
    }

    pub(crate) fn extend_skill_roots(
        self,
        catalog: &mut crate::engine::skill::effect::SkillEffectCatalog,
        skill_ids: impl IntoIterator<Item = i32>,
        buff_ids: impl IntoIterator<Item = i32>,
    ) {
        catalog.extend_roots_and_warn(self.game_data, skill_ids, buff_ids);
    }

    pub(crate) fn extend_skill_entities<'a>(
        self,
        catalog: &mut crate::engine::skill::effect::SkillEffectCatalog,
        entities: impl IntoIterator<Item = &'a sonettobuf::FightEntityInfo>,
    ) {
        catalog.extend_entities_and_warn(self.game_data, entities);
    }

    pub(crate) fn defender_reservation_count(self, fight: &sonettobuf::Fight) -> usize {
        configured_defender_reservation_count(self.game_data, fight)
    }

    pub(crate) fn conduit_device(
        self,
        model_id: i32,
    ) -> Result<
        Option<Vec<Vec<crate::engine::manager::conduit::ConduitSkill>>>,
        crate::engine::manager::conduit::ConduitError,
    > {
        configured_conduit_device(self.game_data, model_id)
    }

    pub(crate) fn boss_rush_target_models(
        self,
        episode_id: i32,
        battle_id: i32,
    ) -> Option<Vec<i32>> {
        self.game_data
            .activity128_battle(episode_id, battle_id)
            .map(|route| route.target_model_ids)
    }

    pub(crate) fn battle_max_round(self, battle_id: i32) -> Option<i32> {
        self.game_data
            .battle
            .get(battle_id)
            .map(|battle| battle.max_round)
    }

    pub(crate) fn battle_win_target_model(self, battle_id: i32) -> Option<i32> {
        let mut parts = self
            .game_data
            .battle
            .get(battle_id)?
            .win_condition
            .split('#');
        if parts.next().and_then(|value| value.parse::<i32>().ok()) != Some(3) {
            return None;
        }
        parts.next().and_then(|value| value.parse::<i32>().ok())
    }

    pub(crate) fn wave_start_actions(
        self,
        battle_id: i32,
        wave: i32,
    ) -> Result<
        Vec<crate::engine::fight::trigger::WaveStartAction>,
        crate::engine::fight::trigger::BattleTriggerError,
    > {
        configured_wave_start_actions(self.game_data, battle_id, wave)
    }

    pub(crate) fn careers(self, career: i32) -> Vec<i32> {
        self.game_data
            .fight_effect_group
            .get(career)
            .map(|group| {
                group
                    .career
                    .split('#')
                    .filter_map(|value| value.parse().ok())
                    .collect::<Vec<_>>()
            })
            .filter(|careers| !careers.is_empty())
            .unwrap_or_else(|| vec![career])
    }

    pub(crate) fn model_label(self, model_id: i32) -> i32 {
        self.game_data
            .monster
            .get(model_id)
            .map(|monster| monster.label)
            .unwrap_or_default()
    }

    pub(crate) fn entity_damage_type(self, model_id: i32, entity_type: Option<i32>) -> i32 {
        if entity_type == Some(1) {
            return self
                .game_data
                .character
                .get(model_id)
                .map(|row| row.dmg_type)
                .unwrap_or_default();
        }
        self.game_data
            .monster
            .get(model_id)
            .and_then(|monster| {
                self.game_data
                    .monster_skill_template
                    .get(monster.skill_template)
            })
            .map(|row| row.dmg_type)
            .unwrap_or_default()
    }

    pub(crate) fn monster_resistances(self, model_id: i32) -> Option<MonsterResistances> {
        let monster = self.game_data.monster.get(model_id)?;
        let template = self
            .game_data
            .monster_skill_template
            .get(monster.skill_template)?;
        let resistance = self
            .game_data
            .resistances_attribute
            .get(template.resistance)?;
        Some(MonsterResistances {
            dizzy: resistance.dizzy,
            sleep: resistance.sleep,
            petrified: resistance.petrified,
            frozen: resistance.frozen,
            disarm: resistance.disarm,
            forbid: resistance.forbid,
            seal: resistance.seal,
            cant_get_exskill: resistance.cant_get_exskill,
            del_ex_point: resistance.del_ex_point,
            stress_up: resistance.stress_up,
            control_resilience: resistance.control_resilience,
            del_ex_point_resilience: resistance.del_ex_point_resilience,
            stress_up_resilience: resistance.stress_up_resilience,
            charm: resistance.charm,
        })
    }

    pub(crate) fn entity_base_technic(
        self,
        model_id: i32,
        level: i32,
        entity_type: Option<i32>,
        fallback: i32,
    ) -> i32 {
        if entity_type != Some(1) {
            return fallback;
        }
        self.game_data
            .character_level
            .iter()
            .find(|row| row.hero_id == model_id && row.level == level)
            .map(|row| row.technic)
            .unwrap_or(fallback)
    }

    pub(crate) fn entity_battle_tags(
        self,
        model_id: i32,
        destiny_stone: i32,
        destiny_rank: i32,
    ) -> Vec<i32> {
        let stone_tags = if destiny_stone <= 0 || destiny_rank <= 0 {
            None
        } else {
            self.game_data
                .character_destiny_facets_consume
                .iter()
                .find(|row| row.facets_id == destiny_stone)
                .and_then(|row| {
                    let tags = row
                        .tag
                        .split('#')
                        .filter_map(|tag| tag.parse().ok())
                        .collect::<Vec<_>>();
                    (!tags.is_empty()).then_some(tags)
                })
        };
        let mut tags = stone_tags.unwrap_or_else(|| {
            self.game_data
                .character
                .get(model_id)
                .map(|character| {
                    character
                        .battle_tag
                        .split('#')
                        .filter_map(|tag| tag.parse().ok())
                        .collect()
                })
                .unwrap_or_default()
        });
        tags.sort_unstable();
        tags.dedup();
        tags
    }

    pub(crate) fn entity_ex_attributes(
        self,
        model_id: i32,
        level: Option<i32>,
        entity_type: Option<i32>,
    ) -> EntityExAttributes {
        if entity_type == Some(1) {
            return self
                .game_data
                .character_level
                .iter()
                .find(|row| row.hero_id == model_id && row.level == level.unwrap_or_default())
                .map(|row| EntityExAttributes {
                    crit_rate: row.cri,
                    crit_resist: row.recri,
                    crit_dmg: row.cri_dmg,
                    crit_def: row.cri_def,
                    add_dmg: row.add_dmg,
                    drop_dmg: row.drop_dmg,
                })
                .unwrap_or_default();
        }
        let Some(monster) = self.game_data.monster.get(model_id) else {
            return EntityExAttributes::default();
        };
        if let Some(stats) = crate::engine::entity::stats::monster_instance_ex_stats_with_game_data(
            self.game_data,
            model_id,
            level.unwrap_or_default(),
        ) {
            return EntityExAttributes {
                crit_rate: stats.cri,
                crit_resist: stats.recri,
                crit_dmg: stats.cri_dmg,
                crit_def: stats.cri_def,
                add_dmg: stats.add_dmg,
                drop_dmg: stats.drop_dmg,
            };
        }
        let level = level.unwrap_or(monster.level_true);
        let template_id = if monster.template != 0 {
            monster.template
        } else {
            monster.id
        };
        self.game_data
            .monster_template
            .iter()
            .find(|row| row.template == template_id)
            .map(|row| EntityExAttributes {
                crit_rate: row.cri + row.cri_grow * level,
                crit_resist: row.recri + row.recri_grow * level,
                crit_dmg: row.cri_dmg + row.cri_dmg_grow * level,
                crit_def: row.cri_def + row.cri_def_grow * level,
                add_dmg: row.add_dmg + row.add_dmg_grow * level,
                drop_dmg: row.drop_dmg + row.drop_dmg_grow * level,
            })
            .unwrap_or_default()
    }

    pub(crate) fn battle_rules(
        self,
        fight: &sonettobuf::Fight,
    ) -> Vec<crate::engine::fight::rules::ConfiguredBattleRule> {
        let Some(battle) = self.configured_battle(fight) else {
            return Vec::new();
        };
        battle
            .addition_rule
            .split('|')
            .chain(battle.hidden_rule.split('|'))
            .filter_map(|entry| {
                let (side, rule_id) = entry.split_once('#')?;
                let side =
                    crate::engine::fight::rules::BattleRuleSide::from_id(side.parse().ok()?)?;
                let rule_id = rule_id.parse().ok()?;
                let rule = self.game_data.rule.get(rule_id)?;
                let rule_type =
                    crate::engine::fight::rules::AdditionRuleType::from_id(rule.r#type)?;
                Some((side, rule_id, rule_type, rule.effect.as_str()))
            })
            .flat_map(|(side, rule_id, rule_type, effects)| {
                effects
                    .split(['#', '|'])
                    .filter_map(|skill_id| skill_id.parse::<i32>().ok())
                    .filter(|skill_id| self.game_data.skill.get(*skill_id).is_some())
                    .map(
                        move |skill_id| crate::engine::fight::rules::ConfiguredBattleRule {
                            rule_id,
                            skill_id,
                            side,
                            rule_type,
                        },
                    )
            })
            .collect()
    }

    fn configured_battle(
        self,
        fight: &sonettobuf::Fight,
    ) -> Option<&'static config::battle::Battle> {
        crate::engine::fight::configured_battle_with_game_data(self.game_data, fight)
    }

    fn skill_effect(self, skill_id: i32) -> Option<&'static config::skill_effect::SkillEffect> {
        self.game_data
            .skill_effect
            .get(self.skill_effect_id(skill_id))
    }

    fn card_enchant_ids(
        self,
        enchant_id: i32,
        field: impl FnOnce(&config::card_enchant::CardEnchant) -> &str,
    ) -> Vec<i32> {
        self.game_data
            .card_enchant
            .get(enchant_id)
            .map(field)
            .into_iter()
            .flat_map(|raw| raw.split('#'))
            .filter_map(|id| id.parse().ok())
            .collect()
    }

    fn cloth(self, cloth_id: Option<i32>) -> Option<&'static config::cloth_level::ClothLevel> {
        let cloth_id = cloth_id.unwrap_or(1);
        self.game_data
            .cloth_level
            .iter()
            .find(|cloth| cloth.id == cloth_id && cloth.level == 1)
    }

    pub(crate) fn global() -> Self {
        Self::new(config::configs::get())
    }

    pub(crate) fn try_global() -> Option<Self> {
        config::try_get().map(Self::new)
    }
}

fn magic_circle_definition(
    game_data: &config::GameDB,
    circle_id: i32,
) -> Option<MagicCircleDefinition> {
    let row = game_data.magic_circle.get(circle_id)?;
    Some(MagicCircleDefinition {
        duration: row.round,
        allied_attributes: parse_attribute_pairs(&row.self_attrs),
        enemy_attributes: parse_attribute_pairs(&row.enemy_attrs),
        allied_buffs: parse_positive_ids(&row.self_buff),
        enemy_buffs: parse_positive_ids(&row.enemy_buff),
        self_skills: parse_positive_ids(&row.self_skills),
    })
}

fn parse_attribute_pairs(raw: &str) -> Vec<(i32, i32)> {
    parse_integers(raw)
        .chunks_exact(2)
        .map(|pair| (pair[0], pair[1]))
        .collect()
}

fn parse_positive_ids(raw: &str) -> Vec<i32> {
    parse_integers(raw)
        .into_iter()
        .filter(|id| *id > 0)
        .collect()
}

fn parse_upgrade_ids(raw: &str) -> Vec<i32> {
    raw.split(['|', '#', ','])
        .filter_map(|value| value.trim().parse().ok())
        .filter(|value| *value > 0)
        .collect()
}

fn parse_upgrade_pairs(raw: &str) -> Vec<(i32, i32)> {
    raw.split('|')
        .filter_map(|pair| {
            let mut values = pair.split('#').filter_map(|value| value.parse().ok());
            Some((values.next()?, values.next()?))
        })
        .collect()
}

fn parse_integers(raw: &str) -> Vec<i32> {
    raw.split(['|', '#'])
        .filter_map(|value| value.trim().parse().ok())
        .collect()
}

pub(crate) fn target_count(game_data: &config::GameDB, code: i32) -> i32 {
    game_data
        .ai_monster_target
        .get(code)
        .map(|row| row.target_number)
        .unwrap_or_default()
}

pub(crate) fn configured_wave_start_actions(
    game_data: &config::GameDB,
    battle_id: i32,
    wave: i32,
) -> Result<
    Vec<crate::engine::fight::trigger::WaveStartAction>,
    crate::engine::fight::trigger::BattleTriggerError,
> {
    use crate::engine::fight::trigger::{BattleTriggerError, TriggerActionKind, WaveStartAction};

    let mut actions = Vec::new();
    for trigger in game_data
        .trigger
        .iter()
        .filter(|trigger| trigger.battle_id == battle_id && trigger.trigger_type == "WaveStart")
    {
        let configured_wave =
            trigger
                .param2
                .parse::<i32>()
                .map_err(|_| BattleTriggerError::InvalidWave {
                    trigger_id: trigger.id,
                    value: trigger.param2.clone(),
                })?;
        if configured_wave != wave {
            continue;
        }

        for value in trigger
            .action_list
            .split(['#', '|', ','])
            .filter(|value| !value.is_empty())
        {
            let action_id =
                value
                    .parse::<i32>()
                    .map_err(|_| BattleTriggerError::InvalidActionId {
                        trigger_id: trigger.id,
                        value: value.to_owned(),
                    })?;
            let action = game_data.trigger_action.get(action_id).ok_or(
                BattleTriggerError::MissingAction {
                    trigger_id: trigger.id,
                    action_id,
                },
            )?;
            let kind = match action.action_type.as_str() {
                "Prompt" => {
                    let prompt_id = action.param1.parse::<i32>().map_err(|_| {
                        BattleTriggerError::InvalidPromptId {
                            action_id,
                            value: action.param1.clone(),
                        }
                    })?;
                    if game_data.fight_prompt.get(prompt_id).is_none() {
                        return Err(BattleTriggerError::MissingPrompt {
                            action_id,
                            prompt_id,
                        });
                    }
                    TriggerActionKind::Prompt
                }
                unsupported => {
                    tracing::warn!(
                        target: "battle::engine::fight::trigger",
                        battle_id,
                        wave,
                        trigger_id = trigger.id,
                        action_id,
                        action_type = unsupported,
                        "unsupported battle trigger action"
                    );
                    continue;
                }
            };
            actions.push(WaveStartAction {
                trigger_id: trigger.id,
                action_id,
                kind,
            });
        }
    }
    Ok(actions)
}

pub(crate) fn configured_teaching_cards(
    game_data: &config::GameDB,
    episode_id: i32,
) -> Option<ConfiguredTeachingCards> {
    let row = game_data.teaching_card.get(episode_id)?;
    Some(ConfiguredTeachingCards {
        opening_cards: row.opening_cards.clone(),
        refill_cards: row.refill_cards.clone(),
    })
}

pub(crate) fn configured_device_card_weights(
    game_data: &config::GameDB,
    model_id: i32,
) -> Vec<(i32, usize)> {
    let Some(character) = game_data.character.get(model_id) else {
        return Vec::new();
    };
    let Some(device) = game_data.fight_device.get(character.device_id) else {
        return Vec::new();
    };
    [&device.power_skill, &device.special_power_skill]
        .into_iter()
        .flat_map(|skills| skills.split('|'))
        .filter_map(|entry| {
            let mut parts = entry.split('#');
            let skill_id = parts.next().and_then(|value| value.parse::<i32>().ok());
            let count = parts.next().and_then(|value| value.parse::<usize>().ok());
            match (skill_id, count, parts.next()) {
                (Some(skill_id), Some(count), None) if skill_id > 0 => Some((skill_id, count)),
                _ => None,
            }
        })
        .collect()
}

pub(crate) fn damage_target_count_kind(game_data: &config::GameDB, code: i32) -> i32 {
    match target_count(game_data, code) {
        1 => 1,
        count if count > 1 => 2,
        _ => 0,
    }
}

pub(crate) fn card_enchant_current_hp_loss_permille(
    game_data: &config::GameDB,
    enchant_id: i32,
) -> Option<i32> {
    let feature = &game_data.card_enchant.get(enchant_id)?.feature;
    let parts = feature.split('#').collect::<Vec<_>>();
    let [kind, attacker_rate, defender_rate] = parts.as_slice() else {
        return None;
    };
    if *kind != "burn" {
        return None;
    }
    let attacker_rate = attacker_rate.parse::<i32>().ok()?;
    let defender_rate = defender_rate.parse::<i32>().ok()?;
    (attacker_rate > 0 && attacker_rate == defender_rate).then_some(attacker_rate)
}

pub(crate) fn contract_binding_buffs(
    game_data: &config::GameDB,
    ex_skill_level: i32,
    career: i32,
) -> Option<(i32, i32)> {
    Some((
        mapped_contract_buff(game_data, CONTRACT_OWNER_BUFF_MAP, ex_skill_level, career)?,
        mapped_contract_buff(game_data, CONTRACT_BOUND_BUFF_MAP, ex_skill_level, career)?,
    ))
}

pub(crate) fn summoned_unique_skills(game_data: &config::GameDB, summoned_id: i32) -> Vec<i32> {
    game_data
        .summoned
        .get(summoned_id)
        .into_iter()
        .flat_map(|row| row.unique_skills.split(['#', '|', ',']))
        .filter_map(|value| value.trim().parse().ok())
        .filter(|value| *value > 0)
        .collect()
}

pub(crate) fn configured_ex_point_max(
    game_data: &config::GameDB,
    explicit_max: Option<i32>,
    hero_id: Option<i32>,
    level: i32,
) -> Option<i32> {
    if let Some(max) = explicit_max.filter(|max| *max > 0) {
        return Some(max);
    }

    let hero_id = hero_id?;
    let rank = crate::engine::entity::stats::configured_rank(game_data, hero_id, level);
    let spec = if rank > 2 {
        game_data
            .character_rank_replace
            .get(hero_id)
            .map(|row| row.unique_skill_point.as_str())
    } else {
        None
    }
    .or_else(|| {
        game_data
            .character
            .get(hero_id)
            .map(|row| row.unique_skill_point.as_str())
    })?;

    spec.split('#').nth(1)?.trim().parse().ok()
}

pub(crate) fn configured_monster_toughness(
    game_data: &config::GameDB,
    model_id: i32,
    max_hp: i32,
) -> Option<(i32, i32)> {
    let raw = &game_data.monster.get(model_id)?.toughness;
    let mut parts = raw.split('#').filter_map(|value| value.parse::<i32>().ok());
    let amount = parts.next()?;
    let points = parts.next()?.max(0);
    let show_type = parts.next().unwrap_or_default();
    let segment = match show_type {
        0 => amount,
        1 => (i64::from(max_hp.max(0)) * i64::from(amount) / 1000).clamp(0, i64::from(i32::MAX))
            as i32,
        _ => return None,
    };
    (segment > 0 && points > 0).then_some((segment, points))
}

pub(crate) fn configured_defender_reservation_count(
    game_data: &config::GameDB,
    fight: &sonettobuf::Fight,
) -> usize {
    let Some(battle) = game_data.battle.get(fight.battle_id.unwrap_or_default()) else {
        return 0;
    };
    battle
        .monster_group_ids
        .split('#')
        .filter_map(|id| id.parse::<i32>().ok())
        .filter_map(|id| game_data.monster_group.get(id))
        .map(|group| group.monster.split('#').filter(|id| !id.is_empty()).count())
        .sum()
}

pub(crate) fn configured_conduit_device(
    game_data: &config::GameDB,
    model_id: i32,
) -> Result<
    Option<Vec<Vec<crate::engine::manager::conduit::ConduitSkill>>>,
    crate::engine::manager::conduit::ConduitError,
> {
    use crate::engine::manager::conduit::{ConduitError, ConduitSkill, ConduitSkillGroup};

    let Some(character) = game_data.character.get(model_id) else {
        return Ok(None);
    };
    if character.device_id == 0 {
        return Ok(None);
    }
    let Some(definition) = game_data.fight_device.get(character.device_id) else {
        return Err(ConduitError::MissingDefinition(character.device_id));
    };
    let parse_group = |group, value: &str| {
        value
            .split('|')
            .map(|entry| {
                let parts = entry.split('#').collect::<Vec<_>>();
                if parts.len() != 3 {
                    return Err(ConduitError::InvalidSkill {
                        device_id: character.device_id,
                        group,
                    });
                }
                let parse = |part: &str| {
                    part.parse().map_err(|_| ConduitError::InvalidSkill {
                        device_id: character.device_id,
                        group,
                    })
                };
                Ok(ConduitSkill {
                    skill_id: parse(parts[0])?,
                    cost_type: parse(parts[1])?,
                    cost_value: parse(parts[2])?,
                    is_stopped: false,
                })
            })
            .collect::<Result<Vec<_>, _>>()
    };
    let parse_unique = || {
        definition
            .unique_skill
            .parse()
            .map(|skill_id| {
                vec![ConduitSkill {
                    skill_id,
                    cost_type: 999,
                    cost_value: 0,
                    is_stopped: false,
                }]
            })
            .map_err(|_| ConduitError::InvalidSkill {
                device_id: character.device_id,
                group: ConduitSkillGroup::Unique,
            })
    };

    Ok(Some(vec![
        parse_group(ConduitSkillGroup::Primary, &definition.skill1)?,
        parse_group(ConduitSkillGroup::Secondary, &definition.skill2)?,
        parse_unique()?,
    ]))
}

fn mapped_contract_buff(
    game_data: &config::GameDB,
    config_id: i32,
    ex_skill_level: i32,
    career: i32,
) -> Option<i32> {
    let value = &game_data.fight_const.get(config_id)?.value;
    let levels = value
        .split('|')
        .find_map(|entry| {
            entry
                .split_once('%')
                .filter(|(key, _)| key.parse() == Ok(career))
        })?
        .1;
    levels
        .split(',')
        .find_map(|entry| {
            entry
                .split_once(':')
                .filter(|(key, _)| key.parse() == Ok(ex_skill_level))
        })
        .or_else(|| {
            levels
                .split(',')
                .find_map(|entry| entry.split_once(':').filter(|(key, _)| *key == "0"))
        })?
        .1
        .parse()
        .ok()
}

fn configured_fight_version(raw: Option<&str>) -> ConfiguredFightVersion {
    let Some(raw) = raw else {
        return ConfiguredFightVersion::Missing;
    };
    raw.parse()
        .map(ConfiguredFightVersion::Value)
        .unwrap_or(ConfiguredFightVersion::Invalid)
}

pub(crate) fn impromptu_definition(game_data: &config::GameDB) -> Option<ImpromptuDefinition> {
    Some(ImpromptuDefinition::new(
        game_data.fight_asfd_const.get(5)?.value.parse().ok()?,
        game_data.buff_act.iter().find_map(|act| {
            let definition = crate::engine::skill::buff_act::registry::find(act.id, &act.r#type)?;
            (definition.kind
                == crate::engine::skill::buff_act::registry::BuffActKind::EmitterDamageUp)
                .then_some(definition.key.opcode)
        })?,
        game_data.fight_asfd_const.get(6)?.value.parse().ok()?,
    ))
}

fn lingering_glow_attribute_buff(game_data: &config::GameDB) -> Option<LingeringGlowAttributeBuff> {
    let buff_id = game_data
        .fight_jgz_const
        .get(2)?
        .value
        .parse::<i32>()
        .ok()?;
    let buff = game_data.skill_buff.get(buff_id)?;
    let origin = lingering_glow_attribute_origin(game_data, &buff.features)?;
    Some(LingeringGlowAttributeBuff { buff_id, origin })
}

fn lingering_glow_attribute_origin(
    game_data: &config::GameDB,
    features: &str,
) -> Option<CommandOrigin> {
    features.split('|').find_map(|feature| {
        let values = feature
            .split('#')
            .map(|value| value.trim().parse::<i32>())
            .collect::<Result<Vec<_>, _>>()
            .ok()?;
        let (&act_id, args) = values.split_first()?;
        let act = game_data.buff_act.get(act_id)?;
        let definition = crate::engine::skill::buff_act::registry::find(act_id, &act.r#type)?;
        (definition.kind == crate::engine::skill::buff_act::registry::BuffActKind::AttrByHeatScale
            && definition.supports.is_some_and(|supports| supports(args)))
        .then_some(CommandOrigin {
            domain: RuleDomain::BuffAct,
            key: definition.key,
        })
    })
}

impl std::fmt::Debug for BattleCatalog {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BattleCatalog")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_burn_buff_type() {
        crate::test_support::init_config();

        assert_eq!(
            BattleCatalog::new(crate::test_support::game_data()).burn_buff_type_id(),
            Some(4_150_001)
        );
    }

    #[test]
    fn normalizes_target_counts() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(catalog.target_count(1), 1);
        assert_eq!(catalog.damage_target_count_kind(1), 1);
        assert_eq!(catalog.damage_target_count_kind(201), 2);
        assert_eq!(catalog.damage_target_count_kind(202), 2);
        assert_eq!(catalog.target_count(i32::MAX), 0);
        assert_eq!(catalog.damage_target_count_kind(i32::MAX), 0);
    }

    #[test]
    fn normalizes_summoned_unique_skills() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(catalog.summoned_unique_skills(15_001), vec![30_740_171]);
        assert!(catalog.summoned_unique_skills(-1).is_empty());
    }

    #[test]
    fn normalizes_hero_upgrade_options() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.upgrade_selection(308_664, 3_086_524),
            Some(crate::engine::manager::upgrade::UpgradeSelection {
                upgrade_id: 308_664,
                option_id: 3_086_524,
                add_buff_ids: vec![30_860_132, 30_860_191, 30_860_172, 30_860_112],
                del_buff_ids: vec![30_860_131],
                replace_skill_group1: Vec::new(),
                replace_skill_group2: vec![30_865_127, 30_865_128, 30_865_129],
                replace_big_skill: 0,
                replace_passive_skills: Vec::new(),
                add_passive_skill_ids: Vec::new(),
            })
        );
        assert_eq!(catalog.upgrade_selection(308_664, 3_086_525), None);
        assert_eq!(
            catalog.upgrade_has_available_option(308_664, &[]),
            Some(true)
        );
        assert_eq!(
            catalog.upgrade_has_available_option(308_664, &[3_086_514, 3_086_524, 3_086_534]),
            Some(false)
        );
        assert_eq!(catalog.upgrade_has_available_option(-1, &[]), None);
    }

    #[test]
    fn normalizes_entity_ex_point_max() {
        crate::test_support::init_config();
        let game_data = crate::test_support::game_data();

        assert_eq!(
            configured_ex_point_max(game_data, None, Some(3120), 180),
            Some(8)
        );
        assert_eq!(
            configured_ex_point_max(game_data, Some(17), Some(3120), 180),
            Some(17)
        );
        assert_eq!(configured_ex_point_max(game_data, None, None, 1), None);
    }

    #[test]
    fn normalizes_monster_toughness() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.monster_toughness(109_350_003, 1_015_000),
            Some((101_500, 4))
        );
        assert_eq!(catalog.monster_toughness(-1, 1_015_000), None);
    }

    #[test]
    fn normalizes_lingering_glow_attribute_buff() {
        crate::test_support::init_config();

        assert_eq!(
            BattleCatalog::new(crate::test_support::game_data()).lingering_glow_attribute_buff(),
            Some(LingeringGlowAttributeBuff {
                buff_id: 31340007,
                origin: CommandOrigin {
                    domain: RuleDomain::BuffAct,
                    key: crate::engine::skill::rule::DefinitionKey::new(1053, "AttrByHeatScale"),
                },
            })
        );
    }

    #[test]
    fn normalizes_fight_version() {
        crate::test_support::init_config();
        let game_data = crate::test_support::game_data();

        assert_eq!(
            BattleCatalog::new(game_data).fight_version(),
            ConfiguredFightVersion::Value(
                game_data.r#const.get(1707).unwrap().value.parse().unwrap()
            )
        );
        assert_eq!(
            configured_fight_version(None),
            ConfiguredFightVersion::Missing
        );
        assert_eq!(
            configured_fight_version(Some("not-an-integer")),
            ConfiguredFightVersion::Invalid
        );
    }

    #[test]
    fn normalizes_impromptu_definition() {
        crate::test_support::init_config();
        let game_data = crate::test_support::game_data();
        let catalog = BattleCatalog::new(game_data);
        let definition = catalog.impromptu_definition().unwrap();

        assert_eq!(
            definition.skill_id(),
            game_data
                .fight_asfd_const
                .get(5)
                .unwrap()
                .value
                .parse::<i32>()
                .unwrap()
        );
        assert_eq!(
            game_data
                .buff_act
                .get(definition.damage_up_act_id())
                .unwrap()
                .r#type,
            "EmitterDamageUp"
        );
        assert_eq!(
            definition.damage_rate(2),
            game_data
                .fight_asfd_const
                .get(6)
                .unwrap()
                .value
                .parse::<i32>()
                .unwrap()
                * 2
        );
    }

    #[test]
    fn normalizes_magic_circle_attributes_and_thresholds() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.magic_circle(30001),
            Some(MagicCircleDefinition {
                duration: 3,
                allied_attributes: vec![(205, 150)],
                enemy_attributes: Vec::new(),
                allied_buffs: Vec::new(),
                enemy_buffs: Vec::new(),
                self_skills: Vec::new(),
            })
        );
        assert_eq!(
            catalog.magic_circle_thresholds(),
            vec![
                FieldThreshold {
                    level: 1,
                    progress: 0,
                    definition: FieldDefinition {
                        field_id: 30001,
                        duration: 3,
                    },
                },
                FieldThreshold {
                    level: 2,
                    progress: 50,
                    definition: FieldDefinition {
                        field_id: 30002,
                        duration: 3,
                    },
                },
                FieldThreshold {
                    level: 3,
                    progress: 120,
                    definition: FieldDefinition {
                        field_id: 30003,
                        duration: 2,
                    },
                },
            ]
        );
    }

    #[test]
    fn normalizes_magic_circle_linked_battle_rules() {
        crate::test_support::init_config();

        let blood_domain = BattleCatalog::new(crate::test_support::game_data())
            .magic_circle(100051)
            .unwrap();

        assert_eq!(blood_domain.allied_buffs, vec![308801312]);
        assert_eq!(blood_domain.self_skills, vec![308801821]);
    }

    #[test]
    fn normalizes_buff_consumption_and_action_expiry() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert!(catalog.buff_has_effect_count(6240530));
        assert!(!catalog.buff_has_effect_count(610091));
        assert!(catalog.buff_has_effect_count(90201));
        assert!(!catalog.buff_expires_after_owner_attack(90201));
        assert!(catalog.buff_expires_after_owner_attack(2220010));
        assert!(!catalog.buff_has_effect_count(-1));
        assert!(!catalog.buff_expires_after_owner_attack(-1));
    }

    #[test]
    fn normalizes_configured_buff_features_in_order() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.buff_features(31260151),
            vec![
                ConfiguredBuffFeature {
                    act_type: "CreateMaxHpAdditionalDamageAndRemove".to_owned(),
                    effect_time: 203,
                    effect_condition: 0,
                    raw: "1026#1#750#31260171".to_owned(),
                    values: vec![1026, 1, 750, 31260171],
                },
                ConfiguredBuffFeature {
                    act_type: "SubBuff".to_owned(),
                    effect_time: 0,
                    effect_condition: 0,
                    raw: "933#31260201".to_owned(),
                    values: vec![933, 31260201],
                },
                ConfiguredBuffFeature {
                    act_type: "Bullet".to_owned(),
                    effect_time: 208,
                    effect_condition: 3,
                    raw: "827".to_owned(),
                    values: vec![827],
                },
            ]
        );
        assert!(catalog.buff_features(-1).is_empty());
    }

    #[test]
    fn normalizes_buff_feature_tokens_and_registry_identity() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert!(catalog.buff_has_master_halo(30860161));
        assert!(!catalog.buff_has_master_halo(-1));

        assert_eq!(
            catalog.buff_feature_tokens(109320111),
            vec!["704#1#0", "100#211#50", "100#214#50", "100#206#50",]
        );
        assert_eq!(
            catalog
                .buff_act_definition(704)
                .map(|definition| definition.key),
            Some(crate::engine::skill::rule::DefinitionKey::new(
                704, "HaloBase"
            ))
        );
        assert!(catalog.buff_feature_tokens(-1).is_empty());
        assert!(catalog.buff_act_definition(-1).is_none());
    }

    #[test]
    fn resolves_exact_buff_act_command_origins() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.buff_act_origin(
                1062,
                crate::engine::skill::buff_act::registry::BuffActKind::HeatScaleDecrCounter,
            ),
            Some(CommandOrigin {
                domain: RuleDomain::BuffAct,
                key: crate::engine::skill::rule::DefinitionKey::new(1062, "HeatScaleDecrCounter",),
            })
        );
        assert!(
            catalog
                .buff_act_origin(
                    1062,
                    crate::engine::skill::buff_act::registry::BuffActKind::AddAttrBySpecialCount,
                )
                .is_none()
        );
        assert!(
            catalog
                .buff_act_origin(
                    -1,
                    crate::engine::skill::buff_act::registry::BuffActKind::HeatScaleDecrCounter,
                )
                .is_none()
        );
    }

    #[test]
    fn normalizes_random_buff_pools_in_order() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.buff_pool(30830151),
            Some(vec![308301511, 308301512, 308301513, 308301514])
        );
        assert!(catalog.buff_pool(-1).is_none());
    }

    #[test]
    fn normalizes_card_skill_metadata() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(catalog.skill_effect_id(30020131), 710331);
        assert_eq!(catalog.skill_big_skill_point(30020131), 5);
        assert!(catalog.skill_is_big(30020131));
        assert_eq!(catalog.skill_effect_tag(30020131), 3);
        assert_eq!(catalog.skill_extra_kind(30020131), 0);
        assert_eq!(catalog.skill_type(30020131), 0);
        assert!(catalog.skill_is_attack(30020131));
        assert_eq!(catalog.skill_big_skill_point(30610131), 5);
        assert_eq!(catalog.skill_big_skill_point(31390111), 0);
        assert!(catalog.skill_is_big(30610131));
        assert!(!catalog.skill_is_big(31390111));
        assert_eq!(catalog.skill_effect_tag(31446011), 14);
        assert_eq!(catalog.skill_effect_tag(31390111), 3);
        assert!(catalog.skill_is_ultimate_for_model(31340131, 3134));
        assert!(!catalog.skill_is_ultimate_for_model(31340111, 3134));
        assert!(!catalog.skill_is_ultimate_for_model(31340131, 3139));
        assert!(!catalog.skill_is_ultimate_for_model(-1, 3134));
        assert_eq!(catalog.skill_big_skill_point(-1), 0);
        assert!(!catalog.skill_is_big(-1));
        assert_eq!(catalog.skill_effect_tag(-1), 0);
        assert_eq!(catalog.skill_extra_kind(-1), 0);
        assert_eq!(catalog.skill_type(-1), 0);
        assert!(!catalog.skill_is_attack(-1));
    }

    #[test]
    fn normalizes_player_skills_in_slot_order() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.player_skills(Some(1)),
            vec![
                ConfiguredPlayerSkill {
                    skill_id: 30010201,
                    need_power: Some(40),
                },
                ConfiguredPlayerSkill {
                    skill_id: 30010202,
                    need_power: Some(25),
                },
            ]
        );
        assert_eq!(catalog.player_skills(None), catalog.player_skills(Some(1)));
        assert!(catalog.player_skills(Some(-1)).is_empty());
    }

    #[test]
    fn normalizes_cloth_power_and_skill_terms() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        let fight = sonettobuf::Fight {
            attacker: Some(sonettobuf::FightTeam {
                cloth_id: Some(1),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            catalog.cloth_power(&fight),
            ClothPower::for_fight(crate::test_support::game_data(), &fight)
        );
        assert_eq!(
            catalog.cloth_skill_terms(Some(1), 30010201, 0),
            Some((40, 50, 1))
        );
        assert_eq!(
            catalog.cloth_skill_terms(Some(1), 30010201, 99),
            Some((60, 60, 1))
        );
        assert_eq!(
            catalog.cloth_skill_terms(Some(1), 30010202, 0),
            Some((25, 25, 0))
        );
        assert!(
            catalog
                .cloth_power(&sonettobuf::Fight {
                    attacker: Some(sonettobuf::FightTeam {
                        cloth_id: Some(-1),
                        ..Default::default()
                    }),
                    ..Default::default()
                })
                .is_none()
        );
        assert!(catalog.cloth_skill_terms(Some(1), -1, 0).is_none());
    }

    #[test]
    fn normalizes_damage_affinity_data() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(catalog.fight_const_value(11), 100);
        assert_eq!(catalog.fight_const_value(12), 150);
        assert_eq!(catalog.fight_const_value(13), 300);
        assert_eq!(catalog.fight_const_value(14), 0);
        assert_eq!(catalog.fight_const_value(-1), 0);
        assert_eq!(catalog.career_multiplier(1, 4), 1300);
        assert_eq!(catalog.career_multiplier(1, 1), 1000);
        assert_eq!(catalog.career_multiplier(1, -1), 1000);
        assert_eq!(catalog.career_multiplier(-1, 4), 1000);
        assert_eq!(catalog.strongest_career_multiplier(1), 1300);
        assert_eq!(catalog.strongest_career_multiplier(-1), 1000);
    }

    #[test]
    fn normalizes_current_wave_boss_models() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.boss_model_ids(&sonettobuf::Fight {
                episode_id: Some(90001601),
                ..Default::default()
            }),
            vec![900016101, 900016102]
        );
        assert!(
            catalog
                .boss_model_ids(&sonettobuf::Fight {
                    episode_id: Some(90001601),
                    battle_id: Some(i32::MAX),
                    ..Default::default()
                })
                .is_empty()
        );
        assert!(
            catalog
                .boss_model_ids(&sonettobuf::Fight {
                    episode_id: Some(90001601),
                    cur_wave: Some(2),
                    ..Default::default()
                })
                .is_empty()
        );
    }

    #[test]
    fn normalizes_defender_uid_reservations() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.defender_reservation_count(&sonettobuf::Fight {
                battle_id: Some(9_000_161),
                ..Default::default()
            }),
            2
        );
        assert_eq!(
            catalog.defender_reservation_count(&sonettobuf::Fight {
                episode_id: Some(90_001_601),
                ..Default::default()
            }),
            0
        );
        assert_eq!(
            catalog.defender_reservation_count(&sonettobuf::Fight {
                battle_id: Some(i32::MAX),
                ..Default::default()
            }),
            0
        );
    }

    #[test]
    fn normalizes_boss_rush_target_models() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.boss_rush_target_models(12_800_101, 1_014_201),
            Some(vec![10_142_011])
        );
        assert_eq!(catalog.boss_rush_target_models(12_800_101, 1_014_202), None);
        assert_eq!(catalog.boss_rush_target_models(-1, -1), None);
    }

    #[test]
    fn normalizes_battle_outcome_rules() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(catalog.battle_max_round(1_211), Some(20));
        assert_eq!(catalog.battle_win_target_model(1_211), Some(121_103));
        assert_eq!(catalog.battle_win_target_model(1_001), None);
        assert_eq!(catalog.battle_max_round(-1), None);
        assert_eq!(catalog.battle_win_target_model(-1), None);
    }

    #[test]
    fn normalizes_grouped_careers() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(catalog.careers(101), vec![1, 2]);
        assert_eq!(catalog.careers(1), vec![1]);
        assert_eq!(catalog.careers(-1), vec![-1]);
    }

    #[test]
    fn normalizes_entity_identity_metadata() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(catalog.model_label(900016101), 7);
        assert_eq!(catalog.model_label(-1), 0);
        assert_eq!(catalog.entity_damage_type(3081, Some(1)), 1);
        assert_eq!(catalog.entity_damage_type(3081, Some(2)), 0);
        assert_eq!(catalog.entity_damage_type(900016101, Some(2)), 1);
        assert_eq!(catalog.entity_damage_type(-1, Some(1)), 0);
        assert_eq!(catalog.entity_damage_type(-1, Some(2)), 0);
    }

    #[test]
    fn normalizes_monster_resistances() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.monster_resistances(10_212_111),
            Some(MonsterResistances {
                dizzy: 1000,
                frozen: 2000,
                seal: 1000,
                cant_get_exskill: 1000,
                control_resilience: 2000,
                ..Default::default()
            })
        );
        assert_eq!(catalog.monster_resistances(-1), None);
        assert_eq!(catalog.monster_resistances(900_016_101), None);
    }

    #[test]
    fn normalizes_entity_base_technic() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(catalog.entity_base_technic(3081, 1, Some(1), 99), 273);
        assert_eq!(catalog.entity_base_technic(3081, 2, Some(1), 99), 99);
        assert_eq!(catalog.entity_base_technic(3081, 1, Some(2), 99), 99);
        assert_eq!(catalog.entity_base_technic(-1, 1, Some(1), 99), 99);
    }

    #[test]
    fn normalizes_entity_battle_tags() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(catalog.entity_battle_tags(3081, 0, 0), vec![101, 105, 117]);
        assert_eq!(
            catalog.entity_battle_tags(3081, 308101, 1),
            vec![102, 114, 116]
        );
        assert_eq!(
            catalog.entity_battle_tags(3081, 308101, 0),
            vec![101, 105, 117]
        );
        assert!(catalog.entity_battle_tags(-1, -1, -1).is_empty());
    }

    #[test]
    fn normalizes_entity_ex_attributes() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.entity_ex_attributes(3081, Some(1), Some(1)),
            EntityExAttributes {
                crit_rate: 0,
                crit_resist: 0,
                crit_dmg: 1300,
                crit_def: 0,
                add_dmg: 0,
                drop_dmg: 0,
            }
        );
        assert_eq!(
            catalog.entity_ex_attributes(30110801, Some(170), Some(2)),
            EntityExAttributes {
                crit_rate: 48,
                crit_resist: 0,
                crit_dmg: 1200,
                crit_def: 387,
                add_dmg: 140,
                drop_dmg: 318,
            }
        );
        assert_eq!(
            catalog.entity_ex_attributes(-1, None, Some(1)),
            EntityExAttributes::default()
        );
        assert_eq!(
            catalog.entity_ex_attributes(-1, None, Some(2)),
            EntityExAttributes::default()
        );
    }

    #[test]
    fn normalizes_card_enchant_compatibility() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert!(catalog.card_enchant_excluded_ids(10_001).is_empty());
        assert_eq!(
            catalog.card_enchant_rejected_ids(10_001),
            vec![10_002, 10_005, 10_007, 10_010, 10_011]
        );
        assert_eq!(catalog.card_enchant_rejected_ids(10_006), vec![10_003]);
        assert!(catalog.card_enchant_rejected_ids(-1).is_empty());
        assert_eq!(
            catalog.card_enchant_current_hp_loss_permille(10_002),
            Some(100)
        );
        assert_eq!(catalog.card_enchant_current_hp_loss_permille(10_010), None);
        assert_eq!(catalog.card_enchant_current_hp_loss_permille(-1), None);
    }

    #[test]
    fn normalizes_card_skill_rank() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());
        let card = |skill_id, card_effect| sonettobuf::CardInfo {
            skill_id,
            card_effect,
            ..Default::default()
        };

        assert_eq!(catalog.skill_rank(31_345_111), 1);
        assert_eq!(catalog.skill_rank(-1), 0);
        assert_eq!(catalog.card_skill_rank(&card(Some(31_345_111), Some(9))), 1);
        assert_eq!(catalog.card_skill_rank(&card(Some(-1), Some(9))), 9);
        assert_eq!(catalog.card_skill_rank(&card(None, Some(9))), 9);
    }

    #[test]
    fn normalizes_device_card_weights() {
        crate::test_support::init_config();
        let catalog = BattleCatalog::new(crate::test_support::game_data());

        assert_eq!(
            catalog.device_card_weights(3_149),
            vec![
                (31_446_011, 2),
                (31_446_012, 2),
                (31_446_021, 2),
                (31_446_022, 2),
                (31_490_201, 1),
                (31_490_211, 1),
            ]
        );
        assert!(catalog.device_card_weights(-1).is_empty());
    }

    #[test]
    fn rejects_unsupported_lingering_glow_attribute_buff() {
        crate::test_support::init_config();
        let game_data = crate::test_support::game_data();

        assert_eq!(
            lingering_glow_attribute_origin(game_data, "1053#201#0#1000000"),
            None
        );
        assert!(lingering_glow_attribute_origin(game_data, "1053#201#5#1000000#10000").is_some());
    }
}
