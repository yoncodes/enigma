use crate::engine::manager::buff::BuffStatus;

use super::registry::BuffActKind;

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [status, rounds]
        if BuffStatus::from_id(*status).is_good() && *rounds > 0)
}

pub fn supports_type_id(args: &[i32]) -> bool {
    matches!(args, [type_id, rounds] if *type_id > 0 && *rounds > 0)
}

pub fn duration_delta(kind: Option<BuffActKind>, values: &[i32], status: BuffStatus) -> i32 {
    match (kind, values) {
        (Some(BuffActKind::BuffRoundAdd), [_, configured_status, rounds])
            if supports(&[*configured_status, *rounds])
                && BuffStatus::from_id(*configured_status) == status =>
        {
            *rounds
        }
        _ => 0,
    }
}

pub fn type_duration_delta(kind: Option<BuffActKind>, values: &[i32], type_id: i32) -> i32 {
    match (kind, values) {
        (Some(BuffActKind::BuffRoundAddByBuffTypeId), [_, configured_type_id, rounds])
            if supports_type_id(&[*configured_type_id, *rounds])
                && *configured_type_id == type_id =>
        {
            *rounds
        }
        _ => 0,
    }
}

pub fn extend_duration(duration: i32, delta: i32) -> i32 {
    if duration > 0 {
        duration.saturating_add(delta)
    } else {
        duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_configured_status_gains_rounds() {
        let values = [604, BuffStatus::PositiveStatus as i32, 1];

        assert_eq!(
            duration_delta(
                Some(BuffActKind::BuffRoundAdd),
                &values,
                BuffStatus::PositiveStatus,
            ),
            1
        );
        assert_eq!(
            duration_delta(
                Some(BuffActKind::BuffRoundAdd),
                &values,
                BuffStatus::Counter,
            ),
            0
        );
    }

    #[test]
    fn only_the_configured_buff_type_gains_rounds() {
        let values = [608, 6003, 1];

        assert_eq!(
            type_duration_delta(Some(BuffActKind::BuffRoundAddByBuffTypeId), &values, 6003),
            1
        );
        assert_eq!(
            type_duration_delta(Some(BuffActKind::BuffRoundAddByBuffTypeId), &values, 6002),
            0
        );
    }

    #[test]
    fn permanent_buffs_remain_permanent() {
        assert_eq!(extend_duration(0, 1), 0);
        assert_eq!(extend_duration(2, 1), 3);
    }
}
