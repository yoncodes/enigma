use super::*;

impl ConduitManager {
    pub(crate) fn sync_fight(&self, fight: &mut sonettobuf::Fight) {
        for (team, area) in &self.areas {
            let fight_team = match team {
                1 => fight.attacker.as_mut(),
                2 => fight.defender.as_mut(),
                _ => None,
            };
            if let Some(fight_team) = fight_team {
                fight_team.device_area = Some(area.snapshot());
            }
        }
    }
}

impl ConduitArea {
    pub(crate) fn snapshot(&self) -> sonettobuf::FightDeviceAreaInfo {
        sonettobuf::FightDeviceAreaInfo {
            devices: self
                .devices
                .iter()
                .map(|device| sonettobuf::FightDeviceInfo {
                    skills: device
                        .skill_groups
                        .iter()
                        .map(|group| sonettobuf::FightDeviceSkillGroupInfo {
                            skills: group
                                .iter()
                                .map(|skill| sonettobuf::FightDeviceSkillInfo {
                                    skill_id: Some(skill.skill_id),
                                    cost_type: Some(skill.cost_type),
                                    cost_value: Some(skill.cost_value),
                                    is_stop: Some(skill.is_stopped),
                                })
                                .collect(),
                        })
                        .collect(),
                    index: Some(device.selected_group),
                    uid: Some(device.uid),
                })
                .collect(),
            powers: self
                .powers
                .iter()
                .map(|power| sonettobuf::FightDevicePower {
                    id: Some(power.id),
                    power: Some(power.value),
                })
                .collect(),
        }
    }
}
