pub(super) use common::config::ActivitySchedule;

pub(super) fn get(id: i32) -> Option<&'static ActivitySchedule> {
    common::activity_schedule()
        .binary_search_by_key(&id, |row| row.id)
        .ok()
        .map(|index| &common::activity_schedule()[index])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_schedule_is_sorted_unique_and_current() {
        let rows = common::activity_schedule();
        assert_eq!(rows.len(), 110);
        assert!(rows.windows(2).all(|rows| rows[0].id < rows[1].id));

        let current = [
            (13801, 1790244000000, 1793872799000),
            (13802, 1790244000000, 1793613599000),
            (13803, 1790244000000, 1793613599000),
            (13805, 1790244000000, 1792058399000),
            (13807, 1790244000000, 1793872799000),
            (13809, 1790244000000, 1793872799000),
            (13810, 1790416800000, 1793872799000),
            (13811, 1790244000000, 1792058399000),
            (13814, 1790244000000, 1793872799000),
        ];

        for (id, start_time, end_time) in current {
            let row = get(id).unwrap();
            assert_eq!((row.start_time, row.end_time), (start_time, end_time));
            assert!(row.is_unlock);
        }
        assert!(get(13724).is_none());
    }
}
