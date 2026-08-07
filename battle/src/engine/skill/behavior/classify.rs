#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BehaviorKey {
    pub opcode: i32,
    pub type_name: String,
}

impl BehaviorKey {
    pub fn new(opcode: i32, type_name: impl Into<String>) -> Self {
        Self {
            opcode,
            type_name: type_name.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BehaviorKind {
    AddBuff,
    AddBuffPowerUse,
    AddBuffRanId,
    AddBuffRanTypeId,
    AddBuffByHeroId,
    AddTargetBuffByPoison,
    AddBuffRound,
    AddBuffRound2,
    SupplyShield2,
    SupplyTeamShareShield,
    AddExPoint,
    AddAdrenalineExPoint,
    AddSynchronization,
    EzioProps,
    EzioBigSkillType1,
    EzioBigSkillType2,
    EzioBigSkillEnd,
    EzioBigSkillCheckTimes,
    UltimateExtraAction,
    AttrFix,
    AttrFixByBurnLayerAndExtraBurnHurt,
    AttrFixByLoseHp,
    AttrFixExPoint,
    BulletCritRateAlter,
    CritRateAlter,
    CritRateAlter2,
    MustCrit,
    IgnoreBeatBack,
    DelExPoint,
    DelExPointNotImmunity,
    ChangePower,
    RecoverPower,
    RecoverPowerAndDelCardsUseSkill,
    AddPowerByCritCount,
    TotalSkillRankToPower,
    AddEnergyToCard,
    EnchantHand,
    ChangeHandToTemporary,
    AroundChangeRank,
    CardLevelChange,
    ConsumePowerUpgradeSkillCard,
    AddUniversalCard,
    RedealCardKeepStar2,
    AddQueuedSkillCard,
    AddEmitterEnergy,
    AddTeamEnergy,
    AddRedOrBlueCount,
    AddConduitPower,
    AddConduitExPoint,
    NotifyHeroContract,
    SetConduitSkillGroup,
    StopConduitSkill,
    RaspberryAddCount,
    RaspberryBigSkill,
    SkillRateUp,
    SkillRateUp1,
    SkillRateUp2,
    SkillRateUpCardLevel,
    AddSkillRateByTargetCount,
    SkillRateUpBySelfBuffType,
    ConsumeExPointAddAttr,
    HeatScaleAddSkillRate,
    ConduitRateByConsumedPower,
    ConduitPowerUp,
    ConsumePowerUseSkill,
    ConsumePowerDirectUseSkill,
    ConsumeBuffUseSkill,
    ConsumeBuffUseSkill3,
    ConsumeTargetBuffUseSkill,
    RemoveBuffUseSkill,
    ConsumePowerAddBuff,
    ConsumePowerAddMultiBuff1,
    ConsumeBuffByTypeId,
    ConsumeBuffByTypeId2,
    ConsumeBuffLayerAndOtherAddBuff,
    ConsumeBuffChangeTargets,
    ConsumeBuffUpSkillDamageRate,
    ConsumeBuffAttrFix,
    ConsumeBuffFixMixedRate,
    ConsumeCardAddBuff,
    AddBuffAndAddSpecialCount,
    AddBuffSpecialCount,
    AddSkillRateBySpecialCount,
    ElectricTransform,
    RemoveBuffToAddBuff,
    AddBuffDuration,
    ReduceCastChannelCount,
    Disperse1,
    Disperse2,
    DisperseExclude,
    DisperseForce2,
    Purify1,
    PurifyX,
    DistributeBuff,
    SelfRandomCopyBuffs,
    BuffSortByHp,
    BuffSpread,
    BuffCountMulti,
    ReplaceBuff,
    ReplaceBuff2,
    AddBuffBasedOnEnemyBurnUseCount,
    AddBuffBySkillBuffAdditions,
    AddBuffByBuffLayer,
    AddBuffByBuffLayerRange,
    AbsorbExPoint,
    BloodPoolMaxChange,
    BloodPoolValueChange,
    ConsumeBloodAddBuff,
    ConsumeBloodAddBuff2,
    HeatScaleUseSkillAddCount,
    AddHeatScaleFromBuff,
    AddMagicCircle,
    RemoveMagicCircleById,
    MagicCircleAttr,
    AddSummoned,
    ChangeSummonedLevel,
    AddSummonedLevel,
    RemoveSummoned,
    Summon,
    SummonSp2,
    Kill,
    LethalHpLoss,
    KillTargets,
    MonsterChange,
    Assassinate,
    AverageLife,
    ShellAssign,
    ShellRecycle,
    ShellUseSkill,
    NotifyUpgradeHero,
    IgnoreSkillConfigDamageRate,
    ClientEffect,
    ChangeScene,
    CareerRatioFix,
    ChangeAttackCareer,
    AddAct,
    AddActHero,
    AddActAndCardLimit,
    Damage,
    Damage2,
    OriginDamage,
    OriginDamageCanCrit,
    OriginDamageByTeamAttr,
    OriginDamageByAttrAndBuffGroupSize,
    ButterflyDamage,
    OriginDamageFromInjuryBankBuff,
    RealDamageSelfAndAddBuffToTarget,
    ClearInjuryBankBuffOriginDamage,
    CatapultBuff,
    PoisonConvertToTargetBuff,
    ConsumePoisonSettleDeadlyPoison,
    Heal,
    HealCantCrit,
    HealByTwoAttr,
    Bloodlust,
    Detonate2,
    LostLife,
    ToughnessOverflowDamage,
    ToughnessRecover,
    LostAllLifeByAttr,
    DamageRealLostLife,
    ConfiguredDamageTarget,
    CreateAdditionalDamageAddBuff,
    NuoDiKaDamage,
    DirectUseSkill,
    DirectUseSkill2,
    DirectUseSkillPrev,
    DirectUseSkillCard,
    DirectUseSkillNoAct,
    DirectUseSkillNoAct2,
    DirectUseSkillNotExtra,
    RandomUseSkill,
    Drive,
    DirectUseBigSkill,
    DirectUseGroupAndStarSkill,
    UseExtraSkill,
    CrystalAddCard,
    ConsumeBuffCreatePrecast,
    AddCardRankNext,
    AddCardRankByEffectTag,
    BufferflyRecordSkill,
    CrystalAddCardRank,
    CrystalAddSkillRate,
    CrystalReuse,
    Unknown,
}

pub fn classify(opcode: i32, type_name: &str) -> BehaviorKind {
    super::registry::find_key(opcode, type_name)
        .map(|definition| definition.kind)
        .unwrap_or(BehaviorKind::Unknown)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviorSpec {
    pub key: BehaviorKey,
    pub kind: BehaviorKind,
}

impl BehaviorSpec {
    pub fn new(opcode: i32, type_name: impl Into<String>) -> Self {
        let key = BehaviorKey::new(opcode, type_name);
        let kind = classify(key.opcode, &key.type_name);
        Self { key, kind }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opcode_specific_bloodpool_and_heat_scale_do_not_collapse() {
        assert_eq!(
            classify(60190, "BloodPoolMaxChange"),
            BehaviorKind::BloodPoolMaxChange
        );
        assert_eq!(
            classify(60191, "BloodPoolValueChange"),
            BehaviorKind::BloodPoolValueChange
        );
        assert_eq!(
            classify(60246, "HeatScaleUseSkillAddCount"),
            BehaviorKind::HeatScaleUseSkillAddCount
        );
        assert_eq!(
            classify(60150, "ConsumePowerAddMultiBuff1"),
            BehaviorKind::ConsumePowerAddMultiBuff1
        );
        assert_eq!(
            classify(60152, "AddEmitterEnergy"),
            BehaviorKind::AddEmitterEnergy
        );
        assert_eq!(
            classify(60153, "AddTeamEnergy"),
            BehaviorKind::AddTeamEnergy
        );
        assert_eq!(classify(10001, "SkillRateUp"), BehaviorKind::SkillRateUp);
        assert_eq!(classify(10004, "AttrFix"), BehaviorKind::AttrFix);
        assert_eq!(
            classify(60033, "AttrFixByLoseHp"),
            BehaviorKind::AttrFixByLoseHp
        );
        assert_eq!(
            classify(60206, "CreateAdditionalDamageAddBuff"),
            BehaviorKind::CreateAdditionalDamageAddBuff
        );
        assert_eq!(
            classify(60221, "IgnoreSkillConfigDamageRate"),
            BehaviorKind::IgnoreSkillConfigDamageRate
        );
        assert_eq!(
            classify(60209, "NuoDiKaDamage"),
            BehaviorKind::NuoDiKaDamage
        );
        assert_eq!(
            classify(40006, "MonsterChange"),
            BehaviorKind::MonsterChange
        );
        assert_eq!(classify(60008, "Summon"), BehaviorKind::Summon);
        assert_eq!(classify(60015, "Kill"), BehaviorKind::Kill);
        assert_eq!(
            classify(60174, "ConsumeExPointAddAttr"),
            BehaviorKind::ConsumeExPointAddAttr
        );
        assert_eq!(
            classify(60182, "SkillRateUpBySelfBuffType"),
            BehaviorKind::SkillRateUpBySelfBuffType
        );
        assert_eq!(classify(20021, "AddBuffRanId"), BehaviorKind::AddBuffRanId);
        assert_eq!(
            classify(60183, "SupplyShield2"),
            BehaviorKind::SupplyShield2
        );
        assert_eq!(
            classify(60175, "DirectUseBigSkill"),
            BehaviorKind::DirectUseBigSkill
        );
        assert_eq!(
            classify(50012, "DirectUseSkillNoAct"),
            BehaviorKind::DirectUseSkillNoAct
        );
        assert_eq!(
            classify(50038, "DirectUseSkillNoAct2"),
            BehaviorKind::DirectUseSkillNoAct2
        );
        assert_ne!(
            classify(50012, "DirectUseSkillNoAct"),
            classify(50038, "DirectUseSkillNoAct2")
        );
        assert_eq!(
            classify(60246, "BloodPoolValueChange"),
            BehaviorKind::Unknown
        );
    }

    #[test]
    fn behavior_names_do_not_admit_unowned_opcodes() {
        assert_eq!(classify(1, "AddBuff"), BehaviorKind::AddBuff);
        assert_eq!(classify(999, "AddBuff"), BehaviorKind::Unknown);
        assert_eq!(classify(20002, "AddExPoint"), BehaviorKind::AddExPoint);
        assert_eq!(classify(999, "AddExPoint"), BehaviorKind::Unknown);
    }

    #[test]
    fn unregistered_exact_key_stays_unknown() {
        assert_eq!(classify(60113, "AddExPointByChance"), BehaviorKind::Unknown);
        assert!(super::super::registry::find_key(60113, "AddExPointByChance").is_none());
    }

    #[test]
    fn descriptors_own_row_damage_phase() {
        let add_buff = crate::engine::skill::effect::ParsedBehavior::from_spec(
            BehaviorSpec::new(1, "AddBuff"),
            Vec::new(),
            Vec::new(),
        );
        let attr_fix = crate::engine::skill::effect::ParsedBehavior::from_spec(
            BehaviorSpec::new(10004, "AttrFix"),
            Vec::new(),
            Vec::new(),
        );

        assert!(crate::engine::skill::behavior::runs_after_row_damage(
            &add_buff
        ));
        assert!(!crate::engine::skill::behavior::runs_after_row_damage(
            &attr_fix
        ));
    }

    #[test]
    fn unknown_behavior_preserves_the_original_key() {
        let spec = BehaviorSpec::new(123, "FutureThing");

        assert_eq!(spec.kind, BehaviorKind::Unknown);
        assert_eq!(spec.key.opcode, 123);
        assert_eq!(spec.key.type_name, "FutureThing");
    }
}
