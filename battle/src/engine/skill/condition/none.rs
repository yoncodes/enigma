use crate::engine::skill::condition::parse::ParsedConditionKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NoneMode {
    Always,
    Unconditional,
    SlotMarker,
    EnterBattle,
    WaveStart,
    BindSetup,
    RoundStart,
    AfterRoundStart,
    CardSetup,
    BeforeApResolve,
    ActionQueueCommitted,
    SkillAction,
    SkillActionStart,
    SkillActionAfterDamage,
    SkillActionAfterHit,
    SkillCast,
    SkillAttack,
    SkillDamage,
    SkillAfterAttack,
    Attacked,
    ToughnessBroken,
    AllyAction,
    ImpromptuResolved,
    EnemyAction,
    ShellDeploy,
    ShellRetrieve,
    RoundEnd,
    SmallRoundEnd,
    RoundEndEntitySettlement,
    RoundEndFinalSettlement,
    RoundEndAfterSettlement,
    Healed,
    SwapIn,
    MemoryShellChange,
    TeamDeath,
    WaveScript,
    UnknownHook,
}

macro_rules! parser {
    ($name:ident, $mode:ident) => {
        pub fn $name(_: i32, _: &str, _: &[String]) -> Option<ParsedConditionKind> {
            Some(ParsedConditionKind::None(NoneMode::$mode))
        }
    };
}

parser!(always, Always);
parser!(unconditional, Unconditional);

pub fn unconditional_without_arguments(
    _: i32,
    _: &str,
    args: &[String],
) -> Option<ParsedConditionKind> {
    args.is_empty()
        .then_some(ParsedConditionKind::None(NoneMode::Unconditional))
}
parser!(enter_battle, EnterBattle);
parser!(round_start, RoundStart);
parser!(after_round_start, AfterRoundStart);
parser!(card_setup, CardSetup);
parser!(before_ap_resolve, BeforeApResolve);
parser!(action_queue_committed, ActionQueueCommitted);
parser!(skill_action, SkillAction);
parser!(skill_action_start, SkillActionStart);
parser!(skill_action_after_damage, SkillActionAfterDamage);
parser!(skill_action_after_hit, SkillActionAfterHit);
parser!(skill_cast, SkillCast);
parser!(skill_after_attack, SkillAfterAttack);
parser!(attacked, Attacked);
parser!(toughness_broken, ToughnessBroken);
parser!(ally_action, AllyAction);
parser!(impromptu_resolved, ImpromptuResolved);
parser!(shell_deploy, ShellDeploy);
parser!(shell_retrieve, ShellRetrieve);
parser!(small_round_end, SmallRoundEnd);
parser!(round_end, RoundEnd);
parser!(round_end_entity_settlement, RoundEndEntitySettlement);
parser!(round_end_final_settlement, RoundEndFinalSettlement);
parser!(round_end_after_settlement, RoundEndAfterSettlement);
parser!(healed, Healed);
