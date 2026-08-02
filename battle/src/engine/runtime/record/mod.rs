use crate::engine::{
    event::payload::BattleEvent, runtime::change::BattleChange, skill::rule::SetupStage,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameOwner {
    SetupSide(SetupSide),
    SetupEntity {
        owner_uid: i64,
    },
    Skill {
        source_uid: i64,
        skill_id: i32,
        card_index: i32,
        target_uid: Option<i64>,
    },
    ConduitAction {
        source_uid: i64,
        group: i32,
        skill_position: i32,
        target_uid: Option<i64>,
    },
    ConduitStopped {
        source_uid: i64,
        group: i32,
    },
    BuffAct {
        owner_uid: i64,
        source_uid: i64,
        buff_uid: i64,
        buff_id: i32,
        key: crate::engine::skill::rule::DefinitionKey,
    },
    BuffRule {
        emitter_uid: i64,
        carrier_buff_uid: i64,
        carrier_buff_id: i32,
        rule: crate::engine::skill::rule::DefinitionKey,
    },
    SetupBuffAct {
        owner_uid: i64,
        buff_id: i32,
        key: crate::engine::skill::rule::DefinitionKey,
    },
    SetupMechanic,
    StageWave {
        wave: i32,
    },
    EventRule,
    EventEffect {
        source_uid: i64,
        target_uid: i64,
    },
    RoundPhase(RoundPhase),
    Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundPhase {
    CardRefill,
    RoundStartEvent,
    RoundStartSettlement,
    RoundStartCapacitySync,
    RoundStartSync,
    EntitySettlement,
    EntityCardCleanup,
    EntityEntrySetup,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetupSide {
    Attacker,
    Defender,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FrameTrigger {
    Active,
    Event(BattleEvent),
    Setup { stage: SetupStage, priority: i32 },
}

#[derive(Debug, Clone, PartialEq)]
pub enum RoundCue {
    EnterFightDeal,
    ClearUniversalCard,
    DealCard1,
    LayerHaloSync {
        buffs: Vec<crate::engine::manager::buff::BuffSyncResult>,
    },
    NextRoundCards {
        cards: Vec<sonettobuf::CardInfo>,
        deck_count: i32,
        team_type: i32,
    },
    DeckCount {
        count: i32,
        team_type: i32,
    },
    DealCards {
        team_type: i32,
    },
    CardInvalid {
        card_index: i32,
        team_type: i32,
        reason: CardInvalidReason,
    },
    CardsCompose {
        team_type: i32,
    },
    RedealHandSync {
        cards: Vec<sonettobuf::CardInfo>,
    },
    SmallRoundEnd {
        team_type: i32,
    },
    ChangeRound {
        round: i32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardInvalidReason {
    Default,
    OpponentsDefeated,
}

impl CardInvalidReason {
    pub const fn config_effect(self) -> i32 {
        match self {
            Self::Default => 0,
            Self::OpponentsDefeated => -1,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FrameItem {
    Change(Box<BattleChange>),
    Child(Box<SemanticFrame>),
    Cue(RoundCue),
}

#[derive(Debug, Clone, PartialEq)]
/// Records causal ownership and nesting of committed changes before projection.
/// Packet code renders this structure without regrouping or reordering it.
pub struct SemanticFrame {
    pub owner: FrameOwner,
    pub trigger: FrameTrigger,
    pub items: Vec<FrameItem>,
}

pub(crate) type FramePath = Vec<usize>;

pub(crate) fn push_root(
    frames: &mut Vec<SemanticFrame>,
    owner: FrameOwner,
    trigger: FrameTrigger,
) -> FramePath {
    let index = frames.len();
    frames.push(SemanticFrame {
        owner,
        trigger,
        items: Vec::new(),
    });
    vec![index]
}

pub(crate) fn push_child(
    frames: &mut [SemanticFrame],
    parent: &[usize],
    owner: FrameOwner,
    trigger: FrameTrigger,
) -> FramePath {
    let parent_frame = frame_mut(frames, parent);
    let index = parent_frame.items.len();
    parent_frame
        .items
        .push(FrameItem::Child(Box::new(SemanticFrame {
            owner,
            trigger,
            items: Vec::new(),
        })));
    parent.iter().copied().chain([index]).collect()
}

pub(crate) fn push_change(frames: &mut [SemanticFrame], path: &[usize], change: BattleChange) {
    frame_mut(frames, path)
        .items
        .push(FrameItem::Change(Box::new(change)));
}

pub(crate) fn event_scope_path(frames: &[SemanticFrame], path: &[usize]) -> FramePath {
    let frame = frame_at(frames, path);

    if !matches!(frame.trigger, FrameTrigger::Active)
        && let Some(setup_entity) = setup_entity_scope_path(frames, path)
    {
        setup_entity
    } else if matches!(frame.owner, FrameOwner::BuffAct { .. }) && path.len() > 1 {
        let mut parent = path.to_vec();
        parent.pop();
        parent
    } else {
        path.to_vec()
    }
}

fn setup_entity_scope_path(frames: &[SemanticFrame], path: &[usize]) -> Option<FramePath> {
    let (&root, children) = path.split_first()?;
    let mut frame = frames.get(root)?;
    let mut prefix = vec![root];
    let mut setup_entity =
        matches!(frame.owner, FrameOwner::SetupEntity { .. }).then(|| prefix.clone());

    for &child in children {
        frame = match frame.items.get(child)? {
            FrameItem::Child(frame) => frame,
            FrameItem::Change(_) | FrameItem::Cue(_) => return None,
        };
        prefix.push(child);
        if matches!(frame.owner, FrameOwner::SetupEntity { .. }) {
            setup_entity = Some(prefix.clone());
        }
    }
    setup_entity
}

fn frame_at<'a>(frames: &'a [SemanticFrame], path: &[usize]) -> &'a SemanticFrame {
    let (&root, children) = path.split_first().expect("a frame path has a root");
    let mut frame = frames.get(root).expect("frame root exists");
    for &child in children {
        frame = match frame.items.get(child).expect("frame child exists") {
            FrameItem::Child(frame) => frame,
            FrameItem::Change(_) | FrameItem::Cue(_) => panic!("frame path points to a leaf"),
        };
    }
    frame
}

pub(crate) fn owner_at_path<'a>(frames: &'a [SemanticFrame], path: &[usize]) -> &'a FrameOwner {
    let (&root, children) = path.split_first().expect("a frame path has a root");
    let mut frame = frames.get(root).expect("frame root exists");
    for &child in children {
        frame = match frame.items.get(child).expect("frame child exists") {
            FrameItem::Child(frame) => frame,
            FrameItem::Change(_) | FrameItem::Cue(_) => panic!("frame path points to a leaf"),
        };
    }
    &frame.owner
}

pub(crate) fn active_skill_scope_path(
    frames: &[SemanticFrame],
    path: &[usize],
) -> Option<FramePath> {
    let (&root, children) = path.split_first()?;
    let mut frame = frames.get(root)?;
    let mut prefix = vec![root];
    let mut action = (matches!(frame.owner, FrameOwner::Skill { .. })
        && matches!(frame.trigger, FrameTrigger::Active))
    .then(|| prefix.clone());

    for &child in children {
        frame = match frame.items.get(child)? {
            FrameItem::Child(frame) => frame,
            FrameItem::Change(_) | FrameItem::Cue(_) => return None,
        };
        prefix.push(child);
        if matches!(frame.owner, FrameOwner::Skill { .. })
            && matches!(frame.trigger, FrameTrigger::Active)
        {
            action = Some(prefix.clone());
        }
    }

    action
}

pub(crate) fn set_skill_target(
    frames: &mut [SemanticFrame],
    path: &[usize],
    resolved_target_uid: Option<i64>,
) {
    let Some(resolved_target_uid) = resolved_target_uid else {
        return;
    };
    if let FrameOwner::Skill { target_uid, .. } = &mut frame_mut(frames, path).owner {
        target_uid.get_or_insert(resolved_target_uid);
    }
    if path.len() > 1
        && let FrameOwner::ConduitAction { target_uid, .. } =
            &mut frame_mut(frames, &path[..path.len() - 1]).owner
    {
        target_uid.get_or_insert(resolved_target_uid);
    }
}

pub(crate) fn push_cue(frames: &mut Vec<SemanticFrame>, cue: RoundCue) {
    push_cues(frames, [cue]);
}

pub(crate) fn push_attributed_cue(frames: &mut Vec<SemanticFrame>, source_uid: i64, cue: RoundCue) {
    frames.push(SemanticFrame {
        owner: FrameOwner::EventEffect {
            source_uid,
            target_uid: 0,
        },
        trigger: FrameTrigger::Active,
        items: vec![FrameItem::Cue(cue)],
    });
}

pub(crate) fn push_cues(frames: &mut Vec<SemanticFrame>, cues: impl IntoIterator<Item = RoundCue>) {
    frames.push(SemanticFrame {
        owner: FrameOwner::Command,
        trigger: FrameTrigger::Active,
        items: cues.into_iter().map(FrameItem::Cue).collect(),
    });
}

fn frame_mut<'a>(frames: &'a mut [SemanticFrame], path: &[usize]) -> &'a mut SemanticFrame {
    let (&root, children) = path.split_first().expect("a frame path has a root");
    let mut frame = frames.get_mut(root).expect("frame root exists");
    for &child in children {
        frame = match frame.items.get_mut(child).expect("frame child exists") {
            FrameItem::Child(frame) => frame,
            FrameItem::Change(_) | FrameItem::Cue(_) => panic!("frame path points to a leaf"),
        };
    }
    frame
}

#[cfg(test)]
mod tests;
