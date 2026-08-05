use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};

use super::{BuffAddArgs, BuffDefinition, BuffRoute, BuffStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffStorage {
    Single,
    Layered,
    Counted,
    SeparateCopies,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistingBuffMatch {
    SameId,
    SameIdAndDuration,
    SharedTypeFamily,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffExclusions {
    pub remove_on_grant: Box<[i32]>,
    pub remove_statuses_on_grant: Box<[i32]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffLifetime {
    pub duration: i32,
    pub take_stage: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UidAllocationPolicy {
    Normal,
    LayerChild,
}

impl UidAllocationPolicy {
    pub(super) fn uses_child_for_apply(self, args: BuffAddArgs) -> bool {
        self == Self::LayerChild && (args.layer > 0 || !args.layer_specified)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuplicateGrant {
    MergeExisting,
    ReplaceExisting,
    KeepExisting,
    AddSeparateCopy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuffPolicy {
    pub id: i32,
    pub effective_type_id: i32,
    pub storage: BuffStorage,
    pub instance_limit: Option<i32>,
    pub same_type_capacity: Option<i32>,
    pub shared_group_capacity: Option<SharedGroupCapacity>,
    pub unresolved_include_entries: Box<[(i32, i32)]>,
    pub match_existing: ExistingBuffMatch,
    pub on_duplicate: DuplicateGrant,
    pub exclusions: BuffExclusions,
    pub lifetime: BuffLifetime,
    pub uid: BuffUidPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SharedGroupCapacity {
    pub group_id: i32,
    pub max_instances: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffUidPolicy {
    pub allocation: UidAllocationPolicy,
    pub normal_reservations_before_reapply: i32,
    pub normal_reservations_after_first_apply: i32,
    pub reserve_after_first_apply: bool,
    pub reserve_before_explicit_layer_apply: bool,
    pub reserve_before_reapply: bool,
    pub reserve_on_layer_refresh: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuffPolicyError {
    MissingDefinition(i32),
    MalformedIncludeTypes(i32),
    UnknownIncludeType { buff_id: i32, include_type: i32 },
    AmbiguousStorage(i32),
}

impl BuffPolicy {
    pub fn include_entry_name(include_type: i32, value: i32) -> String {
        BuffDefinition::include_entry_name(include_type, value)
    }

    pub fn for_buff_id(buff_id: i32) -> Option<Self> {
        Self::try_for_buff_id(buff_id).ok()
    }

    pub fn try_for_buff_id(buff_id: i32) -> Result<Self, BuffPolicyError> {
        static POLICIES: OnceLock<HashMap<i32, Result<BuffPolicy, BuffPolicyError>>> =
            OnceLock::new();
        let db = config::try_get().ok_or(BuffPolicyError::MissingDefinition(buff_id))?;
        POLICIES
            .get_or_init(|| {
                db.skill_buff
                    .all()
                    .iter()
                    .map(|row| {
                        let policy = BuffDefinition::get(row.id)
                            .ok_or(BuffPolicyError::MissingDefinition(row.id))
                            .and_then(|definition| Self::compile(&definition));
                        (row.id, policy)
                    })
                    .collect()
            })
            .get(&buff_id)
            .cloned()
            .unwrap_or(Err(BuffPolicyError::MissingDefinition(buff_id)))
    }

    fn compile(definition: &BuffDefinition) -> Result<Self, BuffPolicyError> {
        validate_include_entries(
            definition.id(),
            definition.include_types_valid(),
            definition.include_entries(),
        )?;
        Ok(Self::from_definition(definition))
    }

    pub(super) fn from_definition(definition: &BuffDefinition) -> Self {
        let shared_type_replacement =
            definition.uses_shared_type_family() && definition.status != BuffStatus::Shield;
        let keep_existing_type_family = definition.keeps_existing_type_family();
        let type_family_match = shared_type_replacement || keep_existing_type_family;
        let separate_copies = !type_family_match && definition.reapplies_as_new();
        let storage = if definition.uses_stack_layer() {
            BuffStorage::Layered
        } else if definition.uses_typed_count() {
            BuffStorage::Counted
        } else if separate_copies {
            BuffStorage::SeparateCopies
        } else {
            BuffStorage::Single
        };
        let match_existing = if type_family_match {
            ExistingBuffMatch::SharedTypeFamily
        } else if storage == BuffStorage::SeparateCopies
            || (storage == BuffStorage::Layered && definition.duration > 0)
        {
            ExistingBuffMatch::SameIdAndDuration
        } else {
            ExistingBuffMatch::SameId
        };
        let on_duplicate = if keep_existing_type_family {
            DuplicateGrant::KeepExisting
        } else if separate_copies {
            DuplicateGrant::AddSeparateCopy
        } else if storage == BuffStorage::Single {
            DuplicateGrant::ReplaceExisting
        } else {
            DuplicateGrant::MergeExisting
        };

        Self {
            id: definition.id(),
            effective_type_id: definition.effective_type_id(),
            storage,
            instance_limit: definition.capped_separate_copy_limit(),
            same_type_capacity: definition.same_type_capacity(),
            shared_group_capacity: definition.shared_group_capacity().map(
                |(group_id, max_instances)| SharedGroupCapacity {
                    group_id,
                    max_instances,
                },
            ),
            unresolved_include_entries: definition.unresolved_include_entries().into(),
            match_existing,
            on_duplicate,
            exclusions: BuffExclusions {
                remove_on_grant: definition.exclude_buff_ids().into(),
                remove_statuses_on_grant: definition.exclude_status_ids().into(),
            },
            lifetime: BuffLifetime {
                duration: definition.duration,
                take_stage: definition.take_stage,
            },
            uid: BuffUidPolicy {
                allocation: if definition.uses_child_uid() {
                    UidAllocationPolicy::LayerChild
                } else {
                    UidAllocationPolicy::Normal
                },
                normal_reservations_before_reapply: definition.normal_reservations_before_reapply(),
                normal_reservations_after_first_apply: definition
                    .normal_reservations_after_first_apply(),
                reserve_after_first_apply: definition.reserves_child_after_first_apply(),
                reserve_before_explicit_layer_apply: definition
                    .reserves_child_before_explicit_layer_apply(),
                reserve_before_reapply: definition.reserves_child_before_reapply(),
                reserve_on_layer_refresh: definition.reserves_child_on_layer_refresh(),
            },
        }
    }

    pub(super) fn matches(&self, active: &super::ActiveBuff, route: BuffRoute) -> bool {
        active.owner_uid == route.target_uid
            && match self.match_existing {
                ExistingBuffMatch::SameId => active.buff.buff_id == Some(route.buff_id),
                ExistingBuffMatch::SameIdAndDuration => {
                    active.buff.buff_id == Some(route.buff_id)
                        && active.buff.duration == Some(self.lifetime.duration)
                }
                ExistingBuffMatch::SharedTypeFamily => {
                    active.type_id == self.effective_type_id
                        && active
                            .definition
                            .as_ref()
                            .is_some_and(BuffDefinition::matches_type_family)
                }
            }
    }
}

fn validate_include_entries(
    buff_id: i32,
    valid: bool,
    entries: &[(i32, i32)],
) -> Result<(), BuffPolicyError> {
    if !valid {
        return Err(BuffPolicyError::MalformedIncludeTypes(buff_id));
    }
    if let Some((include_type, _)) = entries
        .iter()
        .find(|(include_type, _)| !(1..=17).contains(include_type))
    {
        return Err(BuffPolicyError::UnknownIncludeType {
            buff_id,
            include_type: *include_type,
        });
    }
    let storage_types = entries
        .iter()
        .map(|(include_type, _)| *include_type)
        .filter(|include_type| matches!(include_type, 10 | 11 | 12 | 14 | 15 | 17))
        .collect::<HashSet<_>>();
    if storage_types.len() > 1 {
        return Err(BuffPolicyError::AmbiguousStorage(buff_id));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_and_ambiguous_include_policy() {
        assert_eq!(
            validate_include_entries(100, true, &[(99, 0)]),
            Err(BuffPolicyError::UnknownIncludeType {
                buff_id: 100,
                include_type: 99,
            })
        );
        assert_eq!(
            validate_include_entries(100, true, &[(10, 3), (11, 2)]),
            Err(BuffPolicyError::AmbiguousStorage(100))
        );
        assert_eq!(
            validate_include_entries(100, false, &[]),
            Err(BuffPolicyError::MalformedIncludeTypes(100))
        );
    }

    #[test]
    fn reports_only_include_entries_without_runtime_semantics() {
        crate::test_support::init_config();

        assert!(
            BuffPolicy::try_for_buff_id(31170002)
                .unwrap()
                .unresolved_include_entries
                .is_empty()
        );
        let proven_shield_family = BuffPolicy::try_for_buff_id(31170002).unwrap();
        assert_eq!(
            proven_shield_family.match_existing,
            ExistingBuffMatch::SameIdAndDuration
        );
        let shared_type = BuffPolicy::try_for_buff_id(400301).unwrap();
        assert!(shared_type.unresolved_include_entries.is_empty());
        assert_eq!(
            shared_type.match_existing,
            ExistingBuffMatch::SharedTypeFamily
        );
        let exclusive_state = BuffPolicy::try_for_buff_id(500101).unwrap();
        assert!(exclusive_state.unresolved_include_entries.is_empty());
        assert_eq!(
            exclusive_state.match_existing,
            ExistingBuffMatch::SharedTypeFamily
        );
        assert_eq!(exclusive_state.on_duplicate, DuplicateGrant::KeepExisting);
        assert!(
            BuffPolicy::try_for_buff_id(31050145)
                .unwrap()
                .unresolved_include_entries
                .is_empty()
        );
        let same_type_capacity = BuffPolicy::try_for_buff_id(6200501).unwrap();
        assert!(same_type_capacity.unresolved_include_entries.is_empty());
        assert_eq!(same_type_capacity.same_type_capacity, Some(10));
        let beryl_count = BuffPolicy::try_for_buff_id(31130123).unwrap();
        assert!(beryl_count.unresolved_include_entries.is_empty());
        assert_eq!(beryl_count.storage, BuffStorage::Counted);
        assert_eq!(
            BuffPolicy::try_for_buff_id(31080111)
                .unwrap()
                .uid
                .normal_reservations_before_reapply,
            2
        );
        let tower_power = BuffPolicy::try_for_buff_id(130100112).unwrap();
        assert!(tower_power.unresolved_include_entries.is_empty());
        assert_eq!(tower_power.storage, BuffStorage::SeparateCopies);
        assert_eq!(
            tower_power.shared_group_capacity,
            Some(SharedGroupCapacity {
                group_id: 10,
                max_instances: 4,
            })
        );
        assert_eq!(
            BuffPolicy::try_for_buff_id(30950113)
                .unwrap()
                .uid
                .normal_reservations_after_first_apply,
            0
        );
    }
}
