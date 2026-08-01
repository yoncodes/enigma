use crate::engine::{
    manager::{
        buff::BuffCommand, card::CardCommand, conduit::ConduitCommand, emitter::EmitterCommand,
        entity::EntityCommand, entity::EntitySkillCommand, eureka::EurekaCommand,
        ex_point::ExPointCommand, field::FieldCommand, gauge::GaugeCommand, hp::HpCommand,
        injury::InjuryCommand, revive::ReviveCommand, shield::ShieldCommand, summon::SummonCommand,
        toughness::ToughnessCommand, upgrade::UpgradeCommand,
    },
    mechanic::{
        buff_precast::BuffPrecastCommand, field_transfer::FieldTransferCommand,
        nuo_di_ka::NuoDiKaCommand, shell::ShellCommand,
    },
    skill::action::{SkillInvocation, SkillLifecycle},
    skill::buff_act::{
        blood_pool::count_add_ex_point::BloodPoolCountAddExPointCommand,
        raspberry::{AddCountCommand, CapacityCommand},
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct ThresholdSkillCommand {
    pub owner_uid: i64,
    pub buff_uid: i64,
    pub key: crate::engine::skill::rule::DefinitionKey,
    pub threshold: i32,
    pub delta: i32,
    pub invocation: SkillInvocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectMarker {
    pub target_uid: i64,
    pub effect_type: i32,
    pub effect_num: i32,
    pub config_effect: i32,
    pub reserve_id: Option<i64>,
    pub reserve_str: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum BattleCommand {
    Buff(BuffCommand),
    BuffBatch(Vec<BuffCommand>),
    Hp(HpCommand),
    HpBatch(Vec<HpCommand>),
    Toughness(ToughnessCommand),
    Injury(InjuryCommand),
    Revive(ReviveCommand),
    Shield(ShieldCommand),
    ExPoint(ExPointCommand),
    Eureka(EurekaCommand),
    Gauge(GaugeCommand),
    Emitter(EmitterCommand),
    Entity(EntityCommand),
    EntitySkill(EntitySkillCommand),
    Card(CardCommand),
    BuffPrecast(BuffPrecastCommand),
    Conduit(ConduitCommand),
    Field(FieldCommand),
    FieldTransfer(FieldTransferCommand),
    Shell(ShellCommand),
    NuoDiKa(NuoDiKaCommand),
    RaspberryCapacity(CapacityCommand),
    RaspberryAddCount(AddCountCommand),
    BloodPoolCountAddExPoint(BloodPoolCountAddExPointCommand),
    Summon(SummonCommand),
    Upgrade(UpgradeCommand),
    ThresholdSkill(ThresholdSkillCommand),
    BloodtitheSpend(crate::engine::mechanic::bloodtithe::spend::SpendCommand),
}

#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum RuleOp {
    Command(BattleCommand),
    Publish(crate::engine::event::payload::BattleEvent),
    Skill(SkillInvocation),
    SkillLifecycle(SkillLifecycle),
    BeginSkillAction {
        lifecycle: SkillLifecycle,
        cost: ExPointCommand,
    },
    BuffFeatureMarker {
        target_uid: i64,
        effect_type: i32,
        effect_num: i32,
        buff_act_id: i32,
    },
    EffectMarker {
        target_uid: i64,
        effect_type: i32,
        effect_num: i32,
        config_effect: i32,
        reserve_id: Option<i64>,
        reserve_str: Option<String>,
    },
    SceneChange {
        scene_id: i32,
    },
    BuffActTrigger(crate::engine::manager::buff::BuffActTriggerResult),
    BuffActInfoMarker(crate::engine::manager::buff::BuffActInfoMarkerResult),
    MarkBuffActFired {
        owner_uid: i64,
        buff_uid: i64,
        key: crate::engine::skill::rule::DefinitionKey,
    },
    ModifyActiveSkillTargets {
        additional_count: i32,
    },
    NuoDiKaHit(crate::engine::mechanic::nuo_di_ka::NuoDiKaHit),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        manager::buff::{BuffGrant, CommandOrigin},
        skill::action::SkillRequest,
        skill::rule::{DefinitionKey, RuleDomain},
    };

    #[test]
    fn buff_command_keeps_exact_rule_origin() {
        let origin = CommandOrigin {
            domain: RuleDomain::Behavior,
            key: DefinitionKey::new(20001, "AddBuff"),
        };
        let op = RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,
            source_uid: 10,
            target_uid: -1,
            buff_id: 20,
            amount: Some(2),
            occurrences: 1,
            child_uid_reservations: 0,
        })));

        assert!(matches!(
            op,
            RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant)))
                if grant.origin == origin
        ));
    }

    #[test]
    fn skill_output_is_not_a_state_command() {
        assert!(matches!(
            RuleOp::Skill(
                SkillRequest {
                    source_uid: 10,
                    skill_id: 20,
                }
                .into(),
            ),
            RuleOp::Skill(_)
        ));
    }
}
