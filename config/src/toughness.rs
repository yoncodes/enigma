impl crate::GameDB {
    pub fn toughness_passive_skills(&self, toughness_skill: i32) -> Vec<i32> {
        self.toughnessskill
            .iter()
            .find(|row| row.toughnessskill == toughness_skill)
            .into_iter()
            .flat_map(|row| parse_passive_skills(&row.passive_skill))
            .collect()
    }
}

fn parse_passive_skills(value: &str) -> impl Iterator<Item = i32> + '_ {
    value.split(['#', '|']).filter_map(|id| id.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::parse_passive_skills;

    #[test]
    fn parses_toughness_passive_list() {
        assert_eq!(
            parse_passive_skills("116362200#116362201").collect::<Vec<_>>(),
            [116362200, 116362201]
        );
    }
}
