use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// The capture files of one battle, in replay order.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct CapturedBattle {
    pub start: PathBuf,
    pub start_request: Option<PathBuf>,
    // The packet the build metadata is resolved against.
    pub metadata: PathBuf,
    pub start_cloth: Vec<PathBuf>,
    pub rounds: Vec<CapturedRound>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct CapturedRound {
    pub index: i32,
    pub request: PathBuf,
    pub reply: PathBuf,
    pub cloth: Vec<PathBuf>,
}

impl CapturedBattle {
    /// A battle saved as its own folder with fixed file names.
    pub(super) fn from_folder(round_path: &Path) -> anyhow::Result<Self> {
        let parent = round_path.parent().unwrap_or_else(|| Path::new("."));
        let start = ["StartDungeonReply.json", "StartTowerBattleReply.json"]
            .map(|name| parent.join(name))
            .into_iter()
            .find(|path| path.exists())
            .unwrap_or_else(|| parent.join("StartTowerBattleReply.json"));
        let start_request = ["StartDungeonRequest.json", "StartTowerBattleRequest.json"]
            .map(|name| parent.join(name))
            .into_iter()
            .find(|path| path.exists());
        let legacy = super::uses_legacy_round_names(round_path);
        let rounds = super::round_indices(round_path, i32::MAX)?
            .into_iter()
            .map(|index| {
                let (request, reply) = if legacy {
                    (
                        format!("BeginRoundRequest_{index}.json"),
                        format!("BeginRoundReply_{index}.json"),
                    )
                } else {
                    (
                        format!("begin_round_{index}_request.json"),
                        format!("begin_round_{index}.json"),
                    )
                };
                Ok(CapturedRound {
                    index,
                    request: parent.join(request),
                    reply: parent.join(reply),
                    cloth: super::cloth_input_paths(parent, index)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        Ok(Self {
            start,
            start_request,
            metadata: round_path.to_path_buf(),
            start_cloth: super::cloth_input_paths(parent, 0)?,
            rounds,
        })
    }

    /// Every battle in a session timeline, read in place from its timestamped packets.
    pub(super) fn from_session(dir: &Path) -> anyhow::Result<Vec<Self>> {
        let mut files = fs::read_dir(dir)?
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            })
            .collect::<Vec<_>>();
        files.sort();
        Ok(split_session(&files))
    }
}

pub(super) fn is_session_dir(path: &Path) -> bool {
    path.is_dir()
        && fs::read_dir(path).is_ok_and(|entries| {
            entries
                .filter_map(Result::ok)
                .any(|entry| command(&entry.path()).is_some_and(is_start_reply))
        })
}

fn split_session(files: &[PathBuf]) -> Vec<CapturedBattle> {
    let mut battles = Vec::new();
    let mut current: Option<CapturedBattle> = None;
    let mut last_start_request = None;
    let mut cloth = Vec::new();
    let mut request = None;
    for file in files {
        let Some(command) = command(file) else {
            continue;
        };
        match command {
            "StartDungeonRequest" | "StartTowerBattleRequest" => {
                last_start_request = Some(file.clone());
            }
            command if is_start_reply(command) => {
                battles.extend(current.take());
                cloth.clear();
                request = None;
                current = Some(CapturedBattle {
                    start: file.clone(),
                    start_request: last_start_request.take(),
                    metadata: file.clone(),
                    ..Default::default()
                });
            }
            "UseClothSkillRequest" if current.is_some() => cloth.push(file.clone()),
            "BeginRoundRequest" if current.is_some() => request = Some(file.clone()),
            "BeginRoundReply" => {
                if let (Some(battle), Some(request)) = (current.as_mut(), request.take()) {
                    let index = battle.rounds.len() as i32 + 1;
                    battle.rounds.push(CapturedRound {
                        index,
                        request,
                        reply: file.clone(),
                        cloth: std::mem::take(&mut cloth),
                    });
                }
            }
            "EndFightRequest" | "EndFightPush" | "EndFightReply" => {
                battles.extend(current.take());
            }
            _ => {}
        }
    }
    battles.extend(current);
    battles
}

fn is_start_reply(command: &str) -> bool {
    matches!(command, "StartDungeonReply" | "StartTowerBattleReply")
}

// Timeline packets are named `<date>_<time>_<millis>_<sequence>_<Command>.json`.
fn command(path: &Path) -> Option<&str> {
    let stem = path.file_stem()?.to_str()?;
    let mut parts = stem.splitn(5, '_');
    let prefix = [parts.next()?, parts.next()?, parts.next()?, parts.next()?];
    prefix
        .iter()
        .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| parts.next())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_timeline_splits_battles_and_assigns_cloth_to_the_next_round() {
        let files = [
            "20260927_080404_694_000408_StartDungeonRequest.json",
            "20260927_080404_757_000410_StartDungeonReply.json",
            "20260927_080409_543_000415_UseClothSkillRequest.json",
            "20260927_080411_302_000420_BeginRoundRequest.json",
            "20260927_080411_371_000421_BeginRoundReply.json",
            "20260927_080415_000_000422_UseClothSkillRequest.json",
            "20260927_080420_569_000424_BeginRoundRequest.json",
            "20260927_080420_623_000425_BeginRoundReply.json",
            "20260927_080425_182_000460_EndFightPush.json",
            "20260927_080430_000_000470_GetServerTimeReply.json",
            "20260927_080433_212_000474_StartDungeonReply.json",
            "20260927_080439_544_000485_BeginRoundRequest.json",
            "20260927_080439_611_000486_BeginRoundReply.json",
        ]
        .map(PathBuf::from);

        let battles = split_session(&files);

        assert_eq!(battles.len(), 2);
        let first = &battles[0];
        assert_eq!(first.start, files[1]);
        assert_eq!(first.start_request.as_ref(), Some(&files[0]));
        assert_eq!(
            first.rounds,
            vec![
                CapturedRound {
                    index: 1,
                    request: files[3].clone(),
                    reply: files[4].clone(),
                    cloth: vec![files[2].clone()],
                },
                CapturedRound {
                    index: 2,
                    request: files[6].clone(),
                    reply: files[7].clone(),
                    cloth: vec![files[5].clone()],
                },
            ]
        );
        assert_eq!(battles[1].start, files[10]);
        assert_eq!(battles[1].start_request, None);
        assert_eq!(battles[1].rounds.len(), 1);
    }
}
