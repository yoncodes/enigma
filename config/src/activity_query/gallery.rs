use crate::{GameDB, activity128_gallery::Activity128Gallery};

impl GameDB {
    pub fn activity128_gallery(&self, boss_type: i32) -> Option<&Activity128Gallery> {
        self.activity128_gallery
            .iter()
            .find(|row| row.r#type == boss_type)
    }

    pub fn activity128_boss_type(&self, activity_id: i32, stage: i32) -> Option<i32> {
        let mut rows = self
            .activity128_stage
            .iter()
            .filter(|row| row.activity_id == activity_id && row.stage == stage);
        let boss_type = rows.next()?.r#type;
        rows.next().is_none().then_some(boss_type)
    }

    pub fn activity128_gallery_points(&self, boss_type: i32) -> Option<Vec<(i32, i32)>> {
        gallery_points(self.activity128_gallery(boss_type)?)
    }
}

fn gallery_points(row: &Activity128Gallery) -> Option<Vec<(i32, i32)>> {
    let mut awards = row.point_exp.split('|');
    let mut points = Vec::new();
    let mut previous = 0;
    for threshold in row.point.split('|') {
        let threshold = threshold.parse::<i32>().ok()?;
        let award = awards.next()?.parse::<i32>().ok()?;
        if threshold <= previous || award < 0 {
            return None;
        }
        points.push((threshold, award));
        previous = threshold;
    }
    awards.next().is_none().then_some(points)
}

#[cfg(test)]
mod tests;
