pub mod battle;
pub mod state;

pub use battle::BattleState;
use logic::{
    activity::ActivityManager, arcade::ArcadeOutsideManager, bp::BattlePassManager,
    charge::ChargeManager, collection::CollectionManager, command_post::CommandPostManager,
    critter::CritterManager, exploration::ExplorationManager, fairyland::FairylandManager,
    guide::GuideManager, hero::HeroManager, hero_invitation::HeroInvitationManager,
    inventory::InventoryManager, investigate::InvestigateManager, mail::MailManager,
    odyssey::OdysseyManager, preferences::PreferenceManager, profile::ProfileManager,
    red_dot::RedDotManager, room::RoomManager, rouge::RougeManager, sign_in::SignInManager,
    social::SocialManager, stat::StatManager, store::StoreManager, story::StoryManager,
    summon::SummonManager, task::TaskManager, turnback::TurnbackManager, udimo::UdimoManager,
};
pub use state::PlayerState;

#[derive(Debug, Clone)]
pub struct Player {
    pub id: i64,
    pub state: PlayerState,
    pub activity: ActivityManager,
    pub arcade: ArcadeOutsideManager,
    pub battle: BattleState,
    pub battle_pass: BattlePassManager,
    pub charge: ChargeManager,
    pub collection: CollectionManager,
    pub command_post: CommandPostManager,
    pub critter: CritterManager,
    pub exploration: ExplorationManager,
    pub fairyland: FairylandManager,
    pub guide: GuideManager,
    pub hero: HeroManager,
    pub hero_invitation: HeroInvitationManager,
    pub inventory: InventoryManager,
    pub investigate: InvestigateManager,
    pub mail: MailManager,
    pub odyssey: OdysseyManager,
    pub preferences: PreferenceManager,
    pub profile: ProfileManager,
    pub red_dot: RedDotManager,
    pub room: RoomManager,
    pub rouge: RougeManager,
    pub sign_in: SignInManager,
    pub social: SocialManager,
    pub stat: StatManager,
    pub story: StoryManager,
    pub store: StoreManager,
    pub summon: SummonManager,
    pub tasks: TaskManager,
    pub turnback: TurnbackManager,
    pub udimo: UdimoManager,
}

impl Player {
    pub fn new(id: i64, state: PlayerState) -> Self {
        Self {
            id,
            state,
            activity: ActivityManager::new(id),
            arcade: ArcadeOutsideManager::new(id),
            battle: BattleState::default(),
            battle_pass: BattlePassManager::new(id),
            charge: ChargeManager::new(id),
            collection: CollectionManager::new(id),
            command_post: CommandPostManager::new(id),
            critter: CritterManager::new(id),
            exploration: ExplorationManager::new(id),
            fairyland: FairylandManager::new(id),
            guide: GuideManager::new(id),
            hero: HeroManager::new(id),
            hero_invitation: HeroInvitationManager::new(id),
            inventory: InventoryManager::new(id),
            investigate: InvestigateManager::new(id),
            mail: MailManager::new(id),
            odyssey: OdysseyManager::new(id),
            preferences: PreferenceManager::new(id),
            profile: ProfileManager::new(id),
            red_dot: RedDotManager::new(id),
            room: RoomManager::new(id),
            rouge: RougeManager::new(id),
            sign_in: SignInManager::new(id),
            social: SocialManager::new(id),
            stat: StatManager::new(id),
            store: StoreManager::new(id),
            story: StoryManager::new(id),
            summon: SummonManager::new(id),
            tasks: TaskManager::new(id),
            turnback: TurnbackManager::new(id),
            udimo: UdimoManager::new(id),
        }
    }
}
