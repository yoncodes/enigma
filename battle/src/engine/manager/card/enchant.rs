use sonettobuf::CardInfo;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum EnchantedType {
    Frozen = 10_001,
    Burn = 10_002,
    Chaos = 10_003,
    Discard = 10_004,
    Blockade = 10_005,
    Precision = 10_006,
    Depresse = 10_007,
    Rouge2Double = 10_008,
    Rouge2Treasure = 10_009,
    Lorenz = 10_010,
    Ramona = 10_011,
}

impl EnchantedType {
    pub const fn id(self) -> i32 {
        self as i32
    }
}

impl TryFrom<i32> for EnchantedType {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match value {
            10_001 => Ok(Self::Frozen),
            10_002 => Ok(Self::Burn),
            10_003 => Ok(Self::Chaos),
            10_004 => Ok(Self::Discard),
            10_005 => Ok(Self::Blockade),
            10_006 => Ok(Self::Precision),
            10_007 => Ok(Self::Depresse),
            10_008 => Ok(Self::Rouge2Double),
            10_009 => Ok(Self::Rouge2Treasure),
            10_010 => Ok(Self::Lorenz),
            10_011 => Ok(Self::Ramona),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundEndCurrentHpLoss {
    pub owner_uid: i64,
    pub permille: i32,
}

pub fn round_end_current_hp_losses(
    game_data: &config::GameDB,
    cards: &[CardInfo],
) -> Vec<RoundEndCurrentHpLoss> {
    collect_round_end_current_hp_losses(cards, |enchant_id| {
        crate::catalog::card_enchant_current_hp_loss_permille(game_data, enchant_id)
    })
}

pub(crate) fn collect_round_end_current_hp_losses(
    cards: &[CardInfo],
    mut current_hp_loss_permille: impl FnMut(i32) -> Option<i32>,
) -> Vec<RoundEndCurrentHpLoss> {
    let mut losses = Vec::<RoundEndCurrentHpLoss>::new();
    for card in cards {
        let Some(owner_uid) = card.uid else { continue };
        let Some(permille) = card
            .enchants
            .iter()
            .filter_map(|enchant| enchant.enchant_id)
            .find_map(&mut current_hp_loss_permille)
        else {
            continue;
        };
        if let Some(loss) = losses.iter_mut().find(|loss| loss.owner_uid == owner_uid) {
            loss.permille = loss.permille.saturating_add(permille);
        } else {
            losses.push(RoundEndCurrentHpLoss {
                owner_uid,
                permille,
            });
        }
    }
    losses
}

#[cfg(test)]
mod tests {
    use sonettobuf::{CardEnchant, CardInfo};

    use super::{EnchantedType, RoundEndCurrentHpLoss, round_end_current_hp_losses};

    #[test]
    fn values_match_lua_fight_enum() {
        assert_eq!(EnchantedType::Frozen.id(), 10_001);
        assert_eq!(EnchantedType::Lorenz.id(), 10_010);
        assert_eq!(EnchantedType::Ramona.id(), 10_011);
        assert_eq!(EnchantedType::try_from(10_002), Ok(EnchantedType::Burn));
        assert!(EnchantedType::try_from(0).is_err());
    }

    #[test]
    fn scalding_cards_accumulate_configured_current_hp_loss_by_owner() {
        crate::test_support::init_config();
        let card = |owner_uid, enchanted_type: EnchantedType| CardInfo {
            uid: Some(owner_uid),
            enchants: vec![CardEnchant {
                enchant_id: Some(enchanted_type.id()),
                duration: Some(-1),
                ..Default::default()
            }],
            ..Default::default()
        };

        assert_eq!(
            round_end_current_hp_losses(
                crate::test_support::game_data(),
                &[
                    card(10, EnchantedType::Burn),
                    card(11, EnchantedType::Lorenz),
                    card(10, EnchantedType::Burn),
                ]
            ),
            vec![RoundEndCurrentHpLoss {
                owner_uid: 10,
                permille: 200,
            }]
        );
    }
}
