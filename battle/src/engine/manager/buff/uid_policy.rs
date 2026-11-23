use super::{BuffDefinition, BuffManager, BuffRoute};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct UidAllocationPlan {
    lane: UidLane,
    pub(super) uid: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UidLane {
    Normal,
    Child,
    Observe,
    Reserved(i32),
}

pub(super) fn plan(
    manager: &BuffManager,
    route: BuffRoute,
    definition: &BuffDefinition,
    child_uid: bool,
    normal_reservations_before: i32,
    child_reservations_before: i32,
) -> UidAllocationPlan {
    if let Some(uid) = manager.reserved_grant_uid(route.target_uid, route.buff_id) {
        return UidAllocationPlan {
            lane: UidLane::Reserved(route.buff_id),
            uid,
        };
    }

    let mut allocator = allocator_snapshot(manager, route.target_uid);
    for _ in 0..normal_reservations_before.max(0) {
        allocator.next();
    }
    for _ in 0..child_reservations_before.max(0) {
        allocator.next_child();
    }

    if uses_defender_enter_uid_lane(
        manager,
        route.source_uid,
        route.target_uid,
        route.buff_id,
        definition,
    ) {
        let lane_uid = 100000 + route.target_uid.abs() * 2;
        if !allocator.has_reached(lane_uid) {
            return UidAllocationPlan {
                lane: UidLane::Observe,
                uid: lane_uid,
            };
        }
    }

    UidAllocationPlan {
        lane: if child_uid {
            UidLane::Child
        } else {
            UidLane::Normal
        },
        uid: if child_uid {
            allocator.next_child()
        } else {
            allocator.next()
        },
    }
}

pub(super) fn reservations(
    manager: &BuffManager,
    target_uid: i64,
    normal_count: i32,
    child_count: i32,
) -> Vec<UidAllocationPlan> {
    let mut allocator = allocator_snapshot(manager, target_uid);
    let mut plans = Vec::new();
    for _ in 0..normal_count.max(0) {
        plans.push(UidAllocationPlan {
            lane: UidLane::Normal,
            uid: allocator.next(),
        });
    }
    for _ in 0..child_count.max(0) {
        plans.push(UidAllocationPlan {
            lane: UidLane::Child,
            uid: allocator.next_child(),
        });
    }
    plans
}

pub(super) fn normal(
    manager: &BuffManager,
    target_uid: i64,
    child_reservations_before: i32,
) -> UidAllocationPlan {
    let mut allocator = allocator_snapshot(manager, target_uid);
    for _ in 0..child_reservations_before.max(0) {
        allocator.next_child();
    }
    UidAllocationPlan {
        lane: UidLane::Normal,
        uid: allocator.next(),
    }
}

pub(super) fn counted_refreshes(
    manager: &BuffManager,
    target_uid: i64,
    count: i32,
) -> Vec<UidAllocationPlan> {
    if !manager.shared_uid_lane {
        return Vec::new();
    }

    let mut allocator = allocator_snapshot(manager, target_uid);
    (0..count.max(0))
        .map(|_| UidAllocationPlan {
            lane: UidLane::Normal,
            uid: allocator.next(),
        })
        .collect()
}

pub(super) fn children(
    manager: &BuffManager,
    target_uid: i64,
    count: i32,
) -> Vec<UidAllocationPlan> {
    let mut allocator = allocator_snapshot(manager, target_uid);
    (0..count.max(0))
        .map(|_| UidAllocationPlan {
            lane: UidLane::Child,
            uid: allocator.next_child(),
        })
        .collect()
}

pub(super) fn last_was_child(manager: &BuffManager, target_uid: i64) -> bool {
    allocator_snapshot(manager, target_uid).last_was_child()
}

pub(super) fn follows_replaced_halo_copies(
    manager: &BuffManager,
    replaced_uids: &[i64],
    target_uid: i64,
) -> bool {
    let Some(last) = allocator_snapshot(manager, target_uid).last_child() else {
        return false;
    };
    replaced_uids.iter().any(|master_uid| {
        manager
            .buffs
            .iter()
            .filter(|copy| super::layer_halo_carrier_uid(copy) == Some(*master_uid))
            .filter_map(|copy| copy.buff.uid)
            .max()
            == Some(last)
    })
}

pub(super) fn children_after_sequence(
    manager: &BuffManager,
    target_uid: i64,
    child_reservations_before: i32,
    preceding: impl IntoIterator<Item = UidAllocationPlan>,
    count: i32,
) -> Vec<UidAllocationPlan> {
    let mut allocator = allocator_snapshot(manager, target_uid);
    for _ in 0..child_reservations_before.max(0) {
        allocator.next_child();
    }
    for plan in preceding {
        apply_to_allocator(&mut allocator, plan);
    }
    (0..count.max(0))
        .map(|_| UidAllocationPlan {
            lane: UidLane::Child,
            uid: allocator.next_child(),
        })
        .collect()
}

pub(super) fn normals_after_sequence(
    manager: &BuffManager,
    target_uid: i64,
    preceding: impl IntoIterator<Item = UidAllocationPlan>,
    count: i32,
) -> Vec<UidAllocationPlan> {
    let mut allocator = allocator_snapshot(manager, target_uid);
    for plan in preceding {
        apply_to_allocator(&mut allocator, plan);
    }
    (0..count.max(0))
        .map(|_| UidAllocationPlan {
            lane: UidLane::Normal,
            uid: allocator.next(),
        })
        .collect()
}

pub(super) fn commit(manager: &mut BuffManager, target_uid: i64, plan: UidAllocationPlan) -> i64 {
    if let UidLane::Reserved(buff_id) = plan.lane {
        let uid = manager
            .take_reserved_grant_uid(target_uid, buff_id)
            .unwrap_or(plan.uid);
        debug_assert_eq!(uid, plan.uid);
        return uid;
    }
    let allocator = manager.allocator_for(manager.team_type(target_uid).unwrap_or_default());
    let uid = apply_to_allocator(allocator, plan);
    debug_assert_eq!(uid, plan.uid);
    uid
}

fn apply_to_allocator(allocator: &mut super::BuffUidAllocator, plan: UidAllocationPlan) -> i64 {
    match plan.lane {
        UidLane::Normal => allocator.next(),
        UidLane::Child => allocator.next_child(),
        UidLane::Observe => {
            allocator.observe(plan.uid);
            plan.uid
        }
        UidLane::Reserved(_) => plan.uid,
    }
}

fn allocator_snapshot(manager: &BuffManager, target_uid: i64) -> super::BuffUidAllocator {
    if manager.shared_uid_lane {
        manager.attacker.clone()
    } else if manager.team_type(target_uid) == Some(2) {
        manager.defender.clone()
    } else {
        manager.attacker.clone()
    }
}

pub(super) fn uses_defender_enter_uid_lane(
    manager: &BuffManager,
    source_uid: i64,
    target_uid: i64,
    buff_id: i32,
    definition: &BuffDefinition,
) -> bool {
    !manager.shared_uid_lane
        && manager.team_type(target_uid) == Some(2)
        && (source_uid == target_uid && definition.effective_type_id() == 998
            || source_links_add_buff_to_enter(manager, source_uid, buff_id))
}

fn source_links_add_buff_to_enter(manager: &BuffManager, source_uid: i64, buff_id: i32) -> bool {
    manager
        .buffs
        .iter()
        .filter(|active| active.owner_uid == source_uid)
        .any(|active| {
            active.definition.as_ref().is_some_and(|definition| {
                definition.features().iter().any(|feature| {
                    crate::engine::skill::buff_act::add_buff_to_enter::linked_buff_id(
                        feature.kind,
                        &feature.values,
                    ) == Some(buff_id)
                })
            })
        })
}
