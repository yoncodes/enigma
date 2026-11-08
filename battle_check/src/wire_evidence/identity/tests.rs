use super::*;

#[test]
fn sequence_boundaries_do_not_mix_with_timestamps() {
    let index = IdentityIndex {
        boundaries: vec![
            ("000102_StartDungeonReply.json".into(), Ok((7, 10002, 1002))),
            ("000163_StartDungeonReply.json".into(), Ok((7, 10003, 1003))),
            (
                "000345_StartDungeonReply.json".into(),
                Ok((7, 10101, 11011)),
            ),
            (
                "20261031_100000_StartDungeonReply.json".into(),
                Ok((7, 10, 20)),
            ),
        ],
    };
    for (reply, expected) in [
        ("000001_BeginRoundReply.json", None),
        ("20261031_090000_BeginRoundReply.json", None),
        ("000125_BeginRoundReply.json", Some((7, 10002, 1002))),
        ("000178_AutoRoundReply.json", Some((7, 10003, 1003))),
        ("000358_BeginRoundReply.json", Some((7, 10101, 11011))),
        ("20261031_100100_AutoRoundReply.json", Some((7, 10, 20))),
    ] {
        assert_eq!(index.response(Path::new(reply)), Ok(expected));
    }
}

#[test]
fn invalid_boundary_keeps_its_name_and_reason() {
    let error = "setup 000163_StartDungeonReply.json: missing or noninteger fight.episodeId";
    let index = IdentityIndex {
        boundaries: vec![
            ("000102_StartDungeonReply.json".into(), Ok((7, 10002, 1002))),
            ("000163_StartDungeonReply.json".into(), Err(error.into())),
        ],
    };
    assert_eq!(
        index.response(Path::new("000178_BeginRoundReply.json")),
        Err(error.into())
    );
}
