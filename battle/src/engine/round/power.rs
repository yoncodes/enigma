use sonettobuf::Fight;

use crate::catalog::BattleCatalog;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClothPower {
    max: i32,
    use_card: i32,
    move_card: i32,
    compose: i32,
    recovery: Vec<(i32, i32)>,
}

impl ClothPower {
    pub(crate) fn configured(
        max: i32,
        use_card: i32,
        move_card: i32,
        compose: i32,
        recovery: &str,
    ) -> Self {
        Self {
            max,
            use_card,
            move_card,
            compose,
            recovery: parse_recovery(recovery),
        }
    }

    pub fn for_fight(game_data: &config::GameDB, fight: &Fight) -> Option<Self> {
        let cloth_id = fight.attacker.as_ref()?.cloth_id.unwrap_or(1);
        let config = game_data
            .cloth_level
            .iter()
            .find(|cloth| cloth.id == cloth_id && cloth.level == 1)?;

        Some(Self::configured(
            config.max_power,
            config.r#use,
            config.r#move,
            config.compose,
            &config.recover,
        ))
    }

    pub fn initial(game_data: &config::GameDB, fight: &Fight) -> i32 {
        Self::initial_value(Self::for_fight(game_data, fight), fight)
    }

    pub(crate) fn seeded(catalog: BattleCatalog, fight: &Fight) -> i32 {
        Self::initial_value(catalog.cloth_power(fight), fight)
    }

    fn initial_value(rule: Option<Self>, fight: &Fight) -> i32 {
        let Some(rule) = rule else {
            return fight
                .attacker
                .as_ref()
                .and_then(|team| team.power)
                .unwrap_or(20);
        };
        let current = fight
            .attacker
            .as_ref()
            .and_then(|team| team.power)
            .unwrap_or_default();
        rule.add(current, rule.recovery_for(1))
    }

    pub fn card_used(&self, current: i32) -> i32 {
        self.add(current, self.use_card)
    }

    pub fn card_moved(&self, current: i32) -> i32 {
        self.add(current, self.move_card)
    }

    pub fn cards_composed(&self, current: i32, count: usize) -> i32 {
        self.add(current, self.compose * count as i32)
    }

    pub fn recover_round(&self, current: i32, round: i32) -> i32 {
        self.add(current, self.recovery_for(round))
    }

    fn add(&self, current: i32, delta: i32) -> i32 {
        (current + delta).clamp(0, self.max)
    }

    fn recovery_for(&self, round: i32) -> i32 {
        self.recovery
            .iter()
            .filter(|(start, _)| *start <= round)
            .max_by_key(|(start, _)| *start)
            .map(|(_, amount)| *amount)
            .unwrap_or_default()
    }
}

fn parse_recovery(raw: &str) -> Vec<(i32, i32)> {
    raw.split('|')
        .filter_map(|entry| {
            let (round, amount) = entry.split_once('#')?;
            Some((round.parse().ok()?, amount.parse().ok()?))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_recovery_threshold_replaces_the_earlier_one() {
        let power = ClothPower {
            max: 99,
            recovery: parse_recovery("1#3|5#8"),
            ..Default::default()
        };

        assert_eq!(power.recover_round(20, 4), 23);
        assert_eq!(power.recover_round(20, 5), 28);
    }

    #[test]
    fn operation_power_is_clamped_to_the_cloth_cap() {
        let power = ClothPower {
            max: 99,
            use_card: 4,
            compose: 2,
            ..Default::default()
        };

        assert_eq!(power.card_used(97), 99);
        assert_eq!(power.cards_composed(98, 2), 99);
    }
}
