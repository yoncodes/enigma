use super::*;
use serde_json::json;

#[test]
fn reads_only_constant_map_data() {
    assert_eq!(
        read("\u{feff}-- map\nlocal path = {\"asset\",} return {{-1, 2.5, true, false, nil, path[1]},}")
            .unwrap(),
        vec![json!([-1, 2.5, true, false, null, "asset"])],
    );
    for input in [
        "local path={} return os.execute(\"anything\")",
        "local path={} return {path[0]}",
        "local path={} return {path[1]}",
        "local path={} return {} more",
        "local path={} return {\"unterminated}",
    ] {
        assert!(read(input).is_err(), "accepted unsupported input: {input}");
    }
}
