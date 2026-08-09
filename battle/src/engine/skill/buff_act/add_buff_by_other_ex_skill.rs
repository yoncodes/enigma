use crate::engine::{
    event::payload::BattleEvent,
    manager::{
        BattleManagers,
        buff::{BuffCommand, BuffGrant},
    },
    skill::{
        effect::SkillEffectCatalog,
        rule::output::{BattleCommand, RuleOp},
        subscriber::BuffActSubscriber,
    },
};

pub fn supports(args: &[i32]) -> bool {
    matches!(args, [per_moxie, single_target_buff_id, mass_target_buff_id, ..]
        if *per_moxie > 0 && *single_target_buff_id > 0 && *mass_target_buff_id > 0)
}

pub fn rule_ops(
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    event: &BattleEvent,
) -> Option<Vec<RuleOp>> {
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::AddBuffByOtherExSkill,
    ) || !supports(&subscriber.args)
    {
        return None;
    }
    let BattleEvent::AllyAction(action) = event else {
        return Some(Vec::new());
    };
    Some(
        rule_op(
            catalog,
            subscriber,
            action.source_uid,
            action.skill_id,
            action.additional_moxie,
        )
        .into_iter()
        .collect(),
    )
}

pub fn rule_op(
    catalog: &SkillEffectCatalog,
    subscriber: &BuffActSubscriber,
    action_source_uid: i64,
    active_skill_id: i32,
    moxie_spent: i32,
) -> Option<RuleOp> {
    if !super::subscriber_is_kind(
        subscriber,
        super::registry::BuffActKind::AddBuffByOtherExSkill,
    ) || subscriber.owner_uid == action_source_uid
        || !catalog.is_big_skill(active_skill_id)
        || moxie_spent <= 0
    {
        return None;
    }
    let [per_moxie, single_target_buff_id, mass_target_buff_id, ..] = subscriber.args.as_slice()
    else {
        return None;
    };
    let buff_id = if catalog.target_limit(active_skill_id) == 1 {
        *single_target_buff_id
    } else {
        *mass_target_buff_id
    };
    let amount = per_moxie.checked_mul(moxie_spent)?;
    if buff_id <= 0 || amount <= 0 {
        return None;
    }
    Some(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(
        BuffGrant {
            origin: super::command_origin(subscriber)?,
            source_uid: if subscriber.source_uid != 0 {
                subscriber.source_uid
            } else {
                subscriber.owner_uid
            },
            target_uid: subscriber.owner_uid,
            buff_id,
            amount: Some(amount),
            occurrences: 1,
            child_uid_reservations: 0,
        },
    ))))
}

pub fn grant_transaction_rule_ops(
    managers: &BattleManagers,
    event: &BattleEvent,
) -> Vec<(crate::engine::manager::buff::ActiveBuffFeature, RuleOp)> {
    super::changed_features(
        managers,
        event,
        super::registry::BuffActKind::AddBuffByOtherExSkill,
    )
    .into_iter()
    .map(|(feature, _)| {
        let op = RuleOp::BuffFeatureMarker {
            target_uid: feature.owner_uid,
            effect_type: sonettobuf::effect_type_enum::EffectType::None as i32,
            effect_num: 0,
            buff_act_id: 0,
        };
        (feature, op)
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{
        event::{kind::EventKind, subscription::SubscriptionKey},
        skill::rule::output::RuleOp,
    };

    fn subscriber() -> BuffActSubscriber {
        BuffActSubscriber {
            owner_uid: 11,
            source_uid: 11,
            buff_uid: 1,
            buff_id: 31140144,
            team_type: 1,
            owner_alive: true,
            amount: 1,
            key: SubscriptionKey::new(
                EventKind::AllyAction,
                crate::engine::skill::rule::DefinitionKey::new(927, "AddBuffByOtherExSkill"),
            ),
            act_type: "AddBuffByOtherExSkill".into(),
            effect_time: 212,
            effect_condition: 0,
            args: vec![1, 31140113, 31140114],
            raw: "927#1#31140113#31140114".into(),
        }
    }

    fn granted(op: RuleOp) -> BuffGrant {
        let RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(grant))) = op else {
            panic!("expected a buff grant");
        };
        grant
    }

    #[test]
    fn other_ultimate_grants_one_narrative_per_spent_moxie() {
        crate::test_support::init_config();
        let catalog = crate::engine::skill::effect::catalog::global();

        let single = granted(rule_op(catalog, &subscriber(), 22, 30630183, 5).unwrap());
        assert_eq!((single.buff_id, single.amount), (31140113, Some(5)));

        let mass = granted(rule_op(catalog, &subscriber(), 22, 31020131, 5).unwrap());
        assert_eq!((mass.buff_id, mass.amount), (31140114, Some(5)));
        assert!(rule_op(catalog, &subscriber(), 11, 30630183, 5).is_none());
    }
}
