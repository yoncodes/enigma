use std::sync::OnceLock;

/// Selects one independently traceable battle-engine subsystem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceArea {
    Damage,
    Drain,
    Gauge,
    Skill,
    Target,
    Buff,
    Condition,
    Event,
}

#[derive(Debug, Default)]
struct TraceFlags {
    all: bool,
    damage: bool,
    drain: bool,
    gauge: bool,
    skill: bool,
    target: bool,
    buff: bool,
    condition: bool,
    event: bool,
}

/// Returns whether diagnostics are enabled for one source-owned subsystem.
///
/// `ENIGMA_BATTLE_TRACE` accepts a comma-separated list such as
/// `damage,skill,target` or `all`. The older subsystem variables remain
/// accepted so existing tracing commands keep working.
pub fn enabled(area: TraceArea) -> bool {
    static FLAGS: OnceLock<TraceFlags> = OnceLock::new();
    let flags = FLAGS.get_or_init(TraceFlags::from_environment);
    flags.all
        || match area {
            TraceArea::Damage => flags.damage,
            TraceArea::Drain => flags.drain,
            TraceArea::Gauge => flags.gauge,
            TraceArea::Skill => flags.skill,
            TraceArea::Target => flags.target,
            TraceArea::Buff => flags.buff,
            TraceArea::Condition => flags.condition,
            TraceArea::Event => flags.event,
        }
}

impl TraceFlags {
    fn from_environment() -> Self {
        let mut flags = Self::parse(
            std::env::var("ENIGMA_BATTLE_TRACE")
                .unwrap_or_default()
                .as_str(),
        );
        flags.damage |= std::env::var_os("ENIGMA_DAMAGE_TRACE").is_some();
        flags.drain |= std::env::var_os("ENIGMA_DRAIN_TRACE").is_some();
        flags.gauge |= std::env::var_os("ENIGMA_GAUGE_TRACE").is_some();
        flags
    }

    fn parse(value: &str) -> Self {
        let mut flags = Self::default();
        for name in value.split(',').map(str::trim) {
            match name.to_ascii_lowercase().as_str() {
                "all" => flags.all = true,
                "damage" => flags.damage = true,
                "drain" => flags.drain = true,
                "gauge" => flags.gauge = true,
                "skill" => flags.skill = true,
                "target" | "targets" => flags.target = true,
                "buff" | "buffs" => flags.buff = true,
                "condition" | "conditions" => flags.condition = true,
                "event" | "events" => flags.event = true,
                "" => {}
                _ => {}
            }
        }
        flags
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn trace_flags_select_independent_source_areas() {
        let flags = TraceFlags::parse("damage, target,BUFF,conditions,events");

        assert!(flags.damage);
        assert!(flags.target);
        assert!(flags.buff);
        assert!(flags.condition);
        assert!(flags.event);
        assert!(!flags.drain);
        assert!(!flags.all);
    }
}
