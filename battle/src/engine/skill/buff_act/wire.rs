use crate::engine::skill::rule::DefinitionKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WirePhase {
    Add,
    Static,
    Refresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuffActWireDefinition {
    pub key: DefinitionKey,
    add: &'static [i32],
    static_read: &'static [i32],
    refresh: &'static [i32],
    pub initial_state: Option<InitialStateRule>,
    pub initial_state_marker: bool,
    pub initial_private_state: Option<InitialPrivateStateRule>,
    pub max_hp: Option<MaxHpWireRule>,
    pub pre_add: Option<WireEffect>,
    pub snapshot_reserve: Option<SnapshotReserveRule>,
    pub refreshes_unchanged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaxHpWireRule {
    pub repeats: u8,
    pub buff_act_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WireEffect {
    pub effect_type: i32,
    pub effect_num: i32,
    pub effect_num1: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialStateRule {
    CrystalSelection,
    ConduitCardSelection,
    ButterflyAllowedSkillKinds,
    HeatScale,
    CurrentHpPermille,
    FirstArgument,
    SecondArgument,
    StringCounter,
    GrantValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitialPrivateStateRule {
    FourthArgument,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotReserveRule {
    ActCommonParamsTail,
}

impl BuffActWireDefinition {
    pub const fn all(key: DefinitionKey, markers: &'static [i32]) -> Self {
        Self::new(key, markers, markers, markers)
    }

    pub const fn new(
        key: DefinitionKey,
        add: &'static [i32],
        static_read: &'static [i32],
        refresh: &'static [i32],
    ) -> Self {
        Self {
            key,
            add,
            static_read,
            refresh,
            initial_state: None,
            initial_state_marker: true,
            initial_private_state: None,
            max_hp: None,
            pre_add: None,
            snapshot_reserve: None,
            refreshes_unchanged: false,
        }
    }

    pub const fn add(key: DefinitionKey, markers: &'static [i32]) -> Self {
        Self {
            key,
            add: markers,
            static_read: &[],
            refresh: &[],
            initial_state: None,
            initial_state_marker: true,
            initial_private_state: None,
            max_hp: None,
            pre_add: None,
            snapshot_reserve: None,
            refreshes_unchanged: false,
        }
    }

    pub const fn add_refresh(key: DefinitionKey, markers: &'static [i32]) -> Self {
        Self {
            key,
            add: markers,
            static_read: &[],
            refresh: markers,
            initial_state: None,
            initial_state_marker: true,
            initial_private_state: None,
            max_hp: None,
            pre_add: None,
            snapshot_reserve: None,
            refreshes_unchanged: false,
        }
    }

    pub const fn with_initial_state(mut self, rule: InitialStateRule) -> Self {
        self.initial_state = Some(rule);
        self
    }

    pub const fn with_embedded_initial_state(mut self, rule: InitialStateRule) -> Self {
        self.initial_state = Some(rule);
        self.initial_state_marker = false;
        self
    }

    pub const fn with_initial_private_state(mut self, rule: InitialPrivateStateRule) -> Self {
        self.initial_private_state = Some(rule);
        self
    }

    pub fn initial_private_state(self, values: &[i32]) -> Option<i32> {
        match self.initial_private_state? {
            InitialPrivateStateRule::FourthArgument => values.get(4).copied(),
        }
    }

    pub const fn with_max_hp(mut self, repeats: u8, buff_act_id: i32) -> Self {
        self.max_hp = Some(MaxHpWireRule {
            repeats,
            buff_act_id,
        });
        self
    }

    pub const fn with_pre_add(mut self, effect: WireEffect) -> Self {
        self.pre_add = Some(effect);
        self
    }

    pub const fn with_snapshot_reserve(mut self, rule: SnapshotReserveRule) -> Self {
        self.snapshot_reserve = Some(rule);
        self
    }

    pub const fn with_unchanged_refresh(mut self) -> Self {
        self.refreshes_unchanged = true;
        self
    }

    pub fn snapshot_reserve_str(self, params: Option<&str>) -> Option<String> {
        match self.snapshot_reserve? {
            SnapshotReserveRule::ActCommonParamsTail => Some(
                params
                    .and_then(|params| params.split_once('#'))
                    .filter(|(act_id, _)| act_id.parse::<i32>().ok() == Some(self.key.opcode))
                    .map(|(_, values)| values)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        }
    }

    pub fn markers(self, phase: WirePhase) -> &'static [i32] {
        match phase {
            WirePhase::Add => self.add,
            WirePhase::Static => self.static_read,
            WirePhase::Refresh => self.refresh,
        }
    }

    pub fn has_output(self) -> bool {
        !self.add.is_empty()
            || !self.static_read.is_empty()
            || !self.refresh.is_empty()
            || self.initial_state.is_some()
            || self.initial_private_state.is_some()
            || self.max_hp.is_some()
            || self.pre_add.is_some()
            || self.snapshot_reserve.is_some()
    }
}

pub fn find(opcode: i32, type_name: &str) -> Option<&'static BuffActWireDefinition> {
    super::registry::find(opcode, type_name)?.wire.as_ref()
}
