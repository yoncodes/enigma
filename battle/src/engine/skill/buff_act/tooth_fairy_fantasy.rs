use crate::engine::{
    event::{kind::EventKind, payload::BattleEvent},
    manager::{BattleManagers, buff::{BuffAccumulateActValue, BuffCommand, BuffConsume, BuffGrant, BuffSelector, DepletedBuff},
        gauge::{GaugeCommand, GaugeOperation}, hp::{HpCommand, HpHeal, HpHealKind}},
    skill::{rule::output::{BattleCommand, RuleOp}, subscriber::BuffActSubscriber, target::TargetPool},
};

pub fn parse_feature(raw: &[String]) -> Option<Vec<i32>> {
    if raw.len() != 11 { return None; }
    let mut out = Vec::new();
    for (i, cell) in raw.iter().enumerate() {
        if i == 5 { for id in cell.split(',') { out.push(id.parse().ok()?); } }
        else { out.push(cell.parse().ok()?); }
    }
    supports(&out).then_some(out)
}
pub fn supports(a: &[i32]) -> bool {
    matches!(a, [tooth, 5, 202, debuff, 6, a, b, c, d, e, 101, 1, 99, 50, inspiration]
        if *tooth > 0 && *debuff > 0 && [*a,*b,*c,*d,*e].iter().all(|v| *v > 0)
            && (*inspiration == 2 || *inspiration == 3))
}
pub fn power_ops(managers: &BattleManagers, pool: &TargetPool, s: &BuffActSubscriber,
    event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !matches!(event, BattleEvent::Kind(EventKind::RoundEndEntitySettlement)) { return Some(vec![]); }
    let [1, 1, 4, buff_id] = s.args.as_slice() else { return None; };
    let origin=super::command_origin(s)?;
    let spent:i32=pool.main_allies(s.owner_uid).iter().map(|ally| managers.eureka.round_change(ally.uid,1).consumed).sum();
    let carry=managers.buff.act_value(s.buff_uid,1085);
    let total=carry.saturating_add(spent);
    let count=total/4; let next=total%4;
    let mut ops=vec![];
    if next != carry { ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(
        BuffAccumulateActValue { origin, target_uid:s.owner_uid,buff_uid:s.buff_uid,act_id:1085,delta:next-carry })))); }
    if count>0 { ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
        origin,source_uid:s.owner_uid,target_uid:s.owner_uid,buff_id:*buff_id,amount:Some(count),occurrences:1,child_uid_reservations:0,
    })))); }
    Some(ops)
}
pub fn round_ops(managers: &BattleManagers, pool: &TargetPool, s: &BuffActSubscriber,
    event: &BattleEvent) -> Option<Vec<RuleOp>> {
    if !matches!(event, BattleEvent::RoundStart | BattleEvent::Kind(EventKind::RoundStart)) { return Some(vec![]); }
    if !supports(&s.args) { return None; }
    let a=&s.args; let origin=super::command_origin(s)?;
    let count=managers.buff.buff_id_amount(s.owner_uid,a[0]).min(a[1]);
    if count<=0 { return Some(vec![]); }
    let mut ops=vec![RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(BuffConsume {
        origin,target_uid:s.owner_uid,selector:BuffSelector::ExactId(a[0]),amount:count,depleted:DepletedBuff::Remove,
    })))];
    for enemy in pool.enemies(s.owner_uid,true).iter().filter(|e| managers.hp.current(e.uid)>0) {
        ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
            origin,source_uid:s.owner_uid,target_uid:enemy.uid,buff_id:a[3],amount:Some(count),occurrences:1,child_uid_reservations:0,
        }))));
    }
    let emitter=crate::engine::manager::emitter::UID;
    if managers.hp.current(emitter)>0 {
        for &buff_id in a[5..10].iter().take(count as usize) {
            ops.push(RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(BuffGrant {
                origin,source_uid:s.owner_uid,target_uid:emitter,buff_id,amount:None,occurrences:1,child_uid_reservations:0,
            }))));
        }
        let key=crate::engine::mechanic::impromptu::inspiration_key(emitter);
        if managers.gauge.get(key).is_some() { ops.push(RuleOp::Command(BattleCommand::Gauge(
            GaugeCommand::new(origin,key,GaugeOperation::ChangeValue { delta:count*a[14] }).attributed_to(s.owner_uid,0)
        ))); }
    }
    for ally in pool.main_allies(s.owner_uid).iter().filter(|e| managers.hp.current(e.uid)>0) {
        let mut missing=managers.hp.max(ally.uid).saturating_sub(managers.hp.current(ally.uid));
        for _ in 0..count {
            let amount=missing.saturating_mul(a[13])/1000; missing-=amount;
            if amount>0 { ops.push(RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(HpHeal {
                origin,source_uid:s.owner_uid,target_uid:ally.uid,amount,config_effect:0,kind:HpHealKind::Normal,
            })))); }
        }
    }
    Some(ops)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_and_preserves_five_damage_buffs_and_portrayal_values() {
        let raw="30530201#5#202#30530204#6#30535601,30535602,30535603,30535604,30535605#101#1#99#50#3";
        let args=raw.split('#').map(str::to_owned).collect::<Vec<_>>();
        let parsed=parse_feature(&args).unwrap();
        assert_eq!(&parsed[5..10], &[30535601,30535602,30535603,30535604,30535605]);
        assert_eq!(parsed[14],3);
        assert!(supports(&parsed));
    }
    #[test]
    fn round_start_consumes_teeth_and_applies_portrayal_five_debuff() {
        use sonettobuf::{BuffInfo, Fight, FightEntityInfo, FightTeam, HeroAttribute};
        let _ = crate::test_support::game_data();
        let fight=Fight { attacker:Some(FightTeam { entitys:vec![FightEntityInfo {
            uid:Some(10),team_type:Some(1),current_hp:Some(500),
            attr:Some(HeroAttribute { hp:Some(1000), ..Default::default() }),
            buffs:vec![BuffInfo { uid:Some(20),buff_id:Some(30530213),from_uid:Some(10), ..Default::default() },
                BuffInfo { uid:Some(21),buff_id:Some(30530201),from_uid:Some(10),layer:Some(3), ..Default::default() }],
            ..Default::default() }],..Default::default() }),
            defender:Some(FightTeam { entitys:vec![FightEntityInfo { uid:Some(-1),team_type:Some(2),
                current_hp:Some(1000),..Default::default() }],..Default::default() }),..Default::default() };
        let managers=BattleManagers::seeded(&fight);
        let pool=TargetPool::from_fight(&fight);
        let feature=managers.buff.active_features_for_owner(&managers.hp,10).into_iter()
            .find(|f| super::super::is_kind(f,super::super::registry::BuffActKind::ToothFairyFantasy)).unwrap();
        let sub=super::super::subscriber_from_feature(feature,EventKind::RoundStart).unwrap();
        let ops=round_ops(&managers,&pool,&sub,&BattleEvent::RoundStart).unwrap();
        assert!(ops.iter().any(|op| matches!(op,RuleOp::Command(BattleCommand::Buff(BuffCommand::Consume(c))) if c.amount==3)));
        assert!(ops.iter().any(|op| matches!(op,RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(g))) if g.target_uid == -1 && g.buff_id==30530204 && g.amount==Some(3))));
        assert_eq!(ops.iter().filter(|op| matches!(op,RuleOp::Command(BattleCommand::Hp(HpCommand::Heal(_))))).count(),3);
    }

    #[test]
    fn round_end_counts_team_eureka_consumption_and_preserves_remainder() {
        use sonettobuf::{BuffInfo,Fight,FightEntityInfo,FightTeam,PowerInfo};
        crate::test_support::init_config();
        let hero=|uid| FightEntityInfo { uid:Some(uid),team_type:Some(1),current_hp:Some(100),
            power_infos:vec![PowerInfo { power_id:Some(1),num:Some(10),max:Some(10) }],..Default::default() };
        let fight=Fight { attacker:Some(FightTeam { entitys:vec![FightEntityInfo {
            buffs:vec![BuffInfo { uid:Some(20),buff_id:Some(30530205),from_uid:Some(10),..Default::default() }],
            ..hero(10) },hero(11)],..Default::default() }),..Default::default() };
        let pool=TargetPool::from_fight(&fight);let mut managers=BattleManagers::seeded(&fight);
        managers.eureka.add(11,11,1,-7,0);
        let f=managers.buff.active_features_for_owner(&managers.hp,10).into_iter()
            .find(|f| super::super::is_kind(f,super::super::registry::BuffActKind::TeamPerPowerAddBuff)).unwrap();
        let sub=super::super::subscriber_from_feature(f,EventKind::RoundEndEntitySettlement).unwrap();
        let ops=power_ops(&managers,&pool,&sub,&BattleEvent::Kind(EventKind::RoundEndEntitySettlement)).unwrap();
        assert!(ops.iter().any(|op| matches!(op,RuleOp::Command(BattleCommand::Buff(BuffCommand::Grant(g))) if g.buff_id==30530201 && g.amount==Some(1))));
        assert!(ops.iter().any(|op| matches!(op,RuleOp::Command(BattleCommand::Buff(BuffCommand::AccumulateActValue(v))) if v.delta==3)));
    }

}
