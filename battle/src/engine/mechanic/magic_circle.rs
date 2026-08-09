use sonettobuf::MagicCircleInfo;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MagicCircleApplyResult {
    pub target_uid: i64,
    pub circle_id: i32,
    pub info: MagicCircleInfo,
}
