use std::cell::RefCell;

use serde::Serialize;

use crate::engine::manager::hp::{HpChanges, HpCommand, HpManager};

type Records = Vec<serde_json::Result<String>>;

thread_local! {
    static RECORDS: RefCell<Option<Records>> = const { RefCell::new(None) };
}

pub fn collect<T>(run: impl FnOnce() -> T) -> serde_json::Result<(T, Vec<String>)> {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            RECORDS.with_borrow_mut(|records| records.take());
        }
    }

    RECORDS.with_borrow_mut(|records| {
        assert!(records.is_none(), "damage export cannot be nested");
        *records = Some(Vec::new());
    });
    let _reset = Reset;
    let result = run();
    let records = RECORDS.with_borrow_mut(|records| records.take().unwrap());
    Ok((result, records.into_iter().collect::<Result<_, _>>()?))
}

#[derive(Serialize)]
struct Record<T> {
    kind: &'static str,
    data: T,
}

pub(super) fn record(kind: &'static str, data: &impl Serialize) {
    RECORDS.with_borrow_mut(|records| {
        if let Some(records) = records {
            records.push(serde_json::to_string(&Record { kind, data }));
        }
    });
}

pub(crate) fn settlement(
    command: HpCommand,
    changes: &HpChanges,
    hp: &HpManager,
    loss_limit: Option<i32>,
) {
    RECORDS.with_borrow_mut(|records| {
        let Some(records) = records else { return };
        let (amount, config_effect, hurt, effect_kind) = match command {
            HpCommand::Damage(hit) | HpCommand::SettleAccumulatedDamage(hit) => {
                (hit.amount, hit.config_effect, hit.hurt, Some(hit.effect_kind))
            }
            HpCommand::Lose(loss) => match loss.hurt {
                Some(hurt) => (loss.amount, loss.config_effect, hurt, None),
                None => return,
            },
            _ => return,
        };
        let (before, after) = changes.hp.map_or_else(
            || {
                let current = hp.current(changes.target_uid);
                (current, current)
            },
            |hp| (hp.before, hp.after),
        );
        let settled_hurt = changes.hp.and_then(|hp| hp.hurt);
        let data = serde_json::json!({
            "skill_id": hurt.skill_id,
            "source_uid": changes.source_uid,
            "target_uid": changes.target_uid,
            "opcode": changes.origin.key.opcode,
            "rule_type": changes.origin.key.type_name,
            "rule_domain": format!("{:?}", changes.origin.domain),
            "config_effect": config_effect,
            "effect_id": hurt.effect_id,
            "buff_act_id": hurt.buff_act_id,
            "buff_uid": hurt.buff_uid,
            "damage_from": format!("{:?}", hurt.damage_from),
            "effect_kind": effect_kind.map(|kind| format!("{kind:?}")),
            "requested": amount,
            "input_reduce_hp": hurt.reduce_hp,
            "reduce_hp": settled_hurt.map(|hurt| hurt.reduce_hp),
            "display_amount": settled_hurt.and_then(|hurt| hurt.display_amount),
            "loss_limit": loss_limit,
            "team_shield_absorbed": changes.team_shared_shield_absorbed.map_or(0, |shield| shield.absorbed),
            "shield_absorbed": changes.shield_absorbed.map_or(0, |shield| shield.absorbed),
            "fake_hp_consumed": changes.fake_hp_consumed.map_or(0, |fake_hp| fake_hp.consumed),
            "hp_before": before,
            "hp_after": after,
            "applied_damage": changes.applied_damage(),
            "publishes_hit": changes.publishes_hit,
        });
        records.push(serde_json::to_string(&Record { kind: "settlement", data }));
    });
}
