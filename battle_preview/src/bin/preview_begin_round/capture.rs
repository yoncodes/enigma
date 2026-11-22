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
    pub start_cloth: Vec<CapturedCloth>,
    pub rounds: Vec<CapturedRound>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct CapturedRound {
    pub index: i32,
    pub request: PathBuf,
    pub reply: PathBuf,
    pub autos: Vec<CapturedAuto>,
    pub cloth: Vec<CapturedCloth>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct CapturedAuto {
    pub request: PathBuf,
    pub reply: PathBuf,
    pub after_cloth: usize,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct CapturedCloth {
    pub request: PathBuf,
    pub reply: Option<PathBuf>,
}

impl CapturedBattle {
    /// A battle saved as its own folder with fixed file names.
    pub(super) fn from_folder(round_path: &Path) -> anyhow::Result<Self> {
        let parent = round_path.parent().unwrap_or_else(|| Path::new("."));
        let start = [
            "StartDungeonReply.json",
            "StartTowerBattleReply.json",
            "StartAct229BattleReply.json",
        ]
        .map(|name| parent.join(name))
        .into_iter()
        .find(|path| path.exists())
        .unwrap_or_else(|| parent.join("StartTowerBattleReply.json"));
        let start_request = [
            "StartDungeonRequest.json",
            "StartTowerBattleRequest.json",
            "StartAct229BattleRequest.json",
        ]
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
                    autos: auto_input(parent, index).into_iter().collect(),
                    cloth: cloth_inputs(parent, index)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        Ok(Self {
            start,
            start_request,
            metadata: round_path.to_path_buf(),
            start_cloth: cloth_inputs(parent, 0)?,
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

    /// The session battle and round selected by one timestamped reply.
    pub(super) fn from_session_reply(path: &Path) -> anyhow::Result<(Self, i32)> {
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        Self::from_session(parent)?
            .into_iter()
            .find_map(|battle| {
                battle
                    .rounds
                    .iter()
                    .find(|round| round.reply == path)
                    .map(|round| round.index)
                    .map(|index| (battle, index))
            })
            .ok_or_else(|| anyhow::anyhow!("{} is not paired in a captured battle", path.display()))
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

pub(super) fn is_session_round_reply(path: &Path) -> bool {
    path.is_file() && command(path) == Some("BeginRoundReply")
}

fn split_session(files: &[PathBuf]) -> Vec<CapturedBattle> {
    let mut battles = Vec::new();
    let mut current: Option<CapturedBattle> = None;
    let mut last_start_request = None;
    let mut cloth = Vec::new();
    let mut request = None;
    let mut auto_request = None;
    let mut autos = Vec::new();
    for file in files {
        let Some(command) = command(file) else {
            continue;
        };
        match command {
            "StartDungeonRequest" | "StartTowerBattleRequest" | "StartAct229BattleRequest" => {
                last_start_request = Some(file.clone());
            }
            command if is_start_reply(command) => {
                battles.extend(current.take());
                cloth.clear();
                request = None;
                auto_request = None;
                autos.clear();
                current = Some(CapturedBattle {
                    start: file.clone(),
                    start_request: last_start_request.take(),
                    metadata: file.clone(),
                    ..Default::default()
                });
            }
            "UseClothSkillRequest" if current.is_some() => cloth.push(CapturedCloth {
                request: file.clone(),
                reply: None,
            }),
            "UseClothSkillReply" if current.is_some() => {
                if let Some(input) = cloth.iter_mut().rev().find(|input| input.reply.is_none()) {
                    input.reply = Some(file.clone());
                }
            }
            "AutoRoundRequest" if current.is_some() => {
                auto_request = Some((file.clone(), cloth.len()));
            }
            "AutoRoundReply" if current.is_some() => {
                if let Some((request, after_cloth)) = auto_request.take() {
                    autos.push(CapturedAuto {
                        request,
                        reply: file.clone(),
                        after_cloth,
                    });
                }
            }
            "BeginRoundRequest" if current.is_some() => request = Some(file.clone()),
            "BeginRoundReply" => {
                if let (Some(battle), Some(request)) = (current.as_mut(), request.take()) {
                    let index = battle.rounds.len() as i32 + 1;
                    battle.rounds.push(CapturedRound {
                        index,
                        request,
                        reply: file.clone(),
                        autos: std::mem::take(&mut autos),
                        cloth: std::mem::take(&mut cloth),
                    });
                }
                auto_request = None;
                autos.clear();
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

fn auto_input(parent: &Path, round_index: i32) -> Option<CapturedAuto> {
    let request = parent.join(format!("AutoRoundRequest_{round_index}.json"));
    let reply = parent.join(format!("AutoRoundReply_{round_index}.json"));
    (request.exists() && reply.exists()).then_some(CapturedAuto {
        request,
        reply,
        after_cloth: 0,
    })
}

fn cloth_inputs(parent: &Path, round_index: i32) -> io::Result<Vec<CapturedCloth>> {
    let mut requests = super::cloth_input_paths(parent, round_index)?;
    let reply_name = format!("UseClothSkillReply_{round_index}");
    let mut replies = fs::read_dir(parent)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| super::is_cloth_input(path, &reply_name))
        .collect::<Vec<_>>();
    replies.sort();
    if requests.is_empty() && !replies.is_empty() {
        let recovered = replies
            .iter()
            .map(|reply| common_cloth_request(parent, reply))
            .collect::<io::Result<Vec<_>>>()?;
        if recovered.iter().all(Option::is_some) {
            requests = recovered.into_iter().flatten().collect();
        }
    }
    if !replies.is_empty() && replies.len() != requests.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "round {round_index} has {} cloth requests but {} replies",
                requests.len(),
                replies.len()
            ),
        ));
    }
    Ok(requests
        .into_iter()
        .enumerate()
        .map(|(index, request)| CapturedCloth {
            request,
            reply: replies.get(index).cloned(),
        })
        .collect())
}

fn common_cloth_request(parent: &Path, reply: &Path) -> io::Result<Option<PathBuf>> {
    let Some(common) = parent
        .parent()
        .and_then(Path::parent)
        .map(|root| root.join("common"))
        .filter(|path| path.is_dir())
    else {
        return Ok(None);
    };
    let expected = fs::read(reply)?;
    let mut files = fs::read_dir(common)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    files.sort();
    let mut request = None;
    let mut matched = None;
    for path in files {
        match command(&path) {
            Some("UseClothSkillRequest") => request = Some(path),
            Some("UseClothSkillReply") => {
                let candidate = request.take();
                if fs::read(&path)? == expected
                    && let Some(candidate) = candidate
                    && matched.replace(candidate).is_some()
                {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("ambiguous common cloth reply: {}", reply.display()),
                    ));
                }
            }
            _ => {}
        }
    }
    Ok(matched)
}

fn is_start_reply(command: &str) -> bool {
    matches!(
        command,
        "StartDungeonReply" | "StartTowerBattleReply" | "StartAct229BattleReply"
    )
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
mod tests;
