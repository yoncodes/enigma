pub use crate::engine::entity::stats::StatInputs;

use sonettobuf::{HeroAttribute, HeroExAttribute, HeroSpAttribute};

pub fn profile_attributes(input: &StatInputs) -> (HeroAttribute, HeroExAttribute, HeroSpAttribute) {
    let stats = crate::engine::entity::stats::Stats::build(input);
    (stats.base(), stats.ex(), stats.sp())
}
