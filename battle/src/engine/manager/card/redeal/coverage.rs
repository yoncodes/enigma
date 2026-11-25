use std::collections::{BTreeMap, HashSet};

use super::{CardInfo, same_card};

pub(super) fn witness(
    pile: &[&CardInfo],
    basics: &[CardInfo],
    slots: usize,
    matches: impl Fn(&CardInfo, usize) -> bool,
) -> Option<Vec<CardInfo>> {
    let mut seen = HashSet::new();
    let basics = basics
        .iter()
        .filter(|card| seen.insert((card.uid, card.skill_id)))
        .collect::<Vec<_>>();
    if !basics
        .iter()
        .all(|basic| (0..slots).any(|index| matches(basic, index)))
    {
        return None;
    }
    let candidates = pile
        .iter()
        .map(|card| {
            (
                *card,
                basics.iter().position(|basic| same_card(card, basic)),
            )
        })
        .collect::<Vec<_>>();
    let available = (0..basics.len())
        .map(|index| candidates.iter().any(|(_, basic)| *basic == Some(index)))
        .collect::<Vec<_>>();
    choose_present(&available, &mut Vec::new(), &|present| {
        find_draws(&candidates, &basics, present, slots, &matches)
    })
}

fn choose_present(
    available: &[bool],
    present: &mut Vec<bool>,
    solve: &impl Fn(&[bool]) -> Option<Vec<CardInfo>>,
) -> Option<Vec<CardInfo>> {
    let Some((&available, remaining)) = available.split_first() else {
        return solve(present);
    };
    for included in [true, false] {
        if included && !available {
            continue;
        }
        present.push(included);
        let result = choose_present(remaining, present, solve);
        present.pop();
        if result.is_some() {
            return result;
        }
    }
    None
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CoverageState {
    seen: Vec<bool>,
    replaced: usize,
}

fn find_draws(
    candidates: &[(&CardInfo, Option<usize>)],
    basics: &[&CardInfo],
    present: &[bool],
    slots: usize,
    matches: &impl Fn(&CardInfo, usize) -> bool,
) -> Option<Vec<CardInfo>> {
    let missing = basics
        .iter()
        .zip(present)
        .rev()
        .filter_map(|(&basic, &present)| (!present).then_some(basic))
        .collect::<Vec<_>>();
    let mut states = BTreeMap::from([(
        CoverageState {
            seen: vec![false; present.len()],
            replaced: 0,
        },
        Vec::new(),
    )]);
    for index in 0..slots {
        let mut next = BTreeMap::new();
        for (state, draws) in states {
            for &(card, basic) in candidates {
                if basic.is_some_and(|basic| !present[basic]) {
                    continue;
                }
                let protected = basic.is_some_and(|basic| !state.seen[basic]);
                if (state.replaced == 0 || protected) && matches(card, index) {
                    let mut kept = state.clone();
                    if let Some(basic) = basic {
                        kept.seen[basic] = true;
                    }
                    next.entry(kept).or_insert_with(|| add_draw(&draws, card));
                }
                if !protected
                    && missing
                        .get(state.replaced)
                        .is_some_and(|basic| matches(basic, index))
                {
                    let mut replaced = state.clone();
                    replaced.replaced += 1;
                    next.entry(replaced)
                        .or_insert_with(|| add_draw(&draws, card));
                }
            }
        }
        if next.is_empty() {
            return None;
        }
        states = next;
    }
    states
        .remove(&CoverageState {
            seen: present.to_vec(),
            replaced: missing.len(),
        })
        .map(|draws| draws.into_iter().cloned().collect())
}

fn add_draw<'a>(draws: &[&'a CardInfo], card: &'a CardInfo) -> Vec<&'a CardInfo> {
    let mut draws = draws.to_vec();
    draws.push(card);
    draws
}
