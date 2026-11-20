pub(super) use common::config::ActivitySchedule;

pub(super) fn get(id: i32) -> Option<&'static ActivitySchedule> {
    #[cfg(test)]
    crate::init_test_server_config();
    common::activity_schedule()
        .binary_search_by_key(&id, |row| row.id)
        .ok()
        .map(|index| &common::activity_schedule()[index])
        .filter(|row| row.enabled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_schedule_is_sorted_unique_and_current() {
        crate::init_test_server_config();
        let rows = common::activity_schedule();
        assert_eq!(rows.len(), 134);
        assert!(rows.windows(2).all(|rows| rows[0].id < rows[1].id));

        let current = [
            (13901, 1793872800000, 1797501599000),
            (13902, 1793872800000, 1797242399000),
            (13903, 1793872800000, 1797501599000),
            (13904, 1793872800000, 1795687199000),
            (13905, 1793872800000, 1795687199000),
            (13906, 1795687200000, 1797501599000),
            (13907, 1793872800000, 1797501599000),
            (13908, 1795168800000, 1796032799000),
            (13909, 1796810400000, 1797501599000),
            (13910, 1795687200000, 1797501599000),
            (13911, 1796032800000, 1797415199000),
            (13912, 1794564000000, 1795773599000),
            (13913, 1793872800000, 1797501599000),
            (13914, 1794045600000, 1797501599000),
            (13915, 1794045600000, 1797501599000),
            (13916, 1793872800000, 1797501599000),
            (13917, 1794045600000, 1797501599000),
            (13918, 1795687200000, 1797501599000),
            (13919, 1795687200000, 1797501599000),
            (13920, 1793872800000, 1797242399000),
            (13921, 1793872800000, 1797242399000),
            (13923, 1796378400000, 1797501599000),
            (13924, 1796896800000, 1797501599000),
            (13930, 1793872800000, 1797501599000),
        ];

        for (id, start_time, end_time) in current {
            let row = get(id).unwrap();
            assert_eq!((row.start_time, row.end_time), (start_time, end_time));
            assert!(row.is_unlock);
        }
        assert!(get(13922).is_none());
    }
}
