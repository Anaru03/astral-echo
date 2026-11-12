use gameplay::domains::mission::MissionState;
use gameplay::events::GameplayEvent;
use gameplay::types::MissionId;
use gameplay::GameplayCommand;

use crate::{
    ecos_anomalia, el_despertar, la_defensa_imposible, tormenta_gravitacional, ProjectEchoSession,
};

pub const CAMPAIGN_ORDER: [MissionId; 5] = [
    el_despertar::EL_DESPERTAR,
    ecos_anomalia::ECOS_DE_LA_ANOMALIA,
    tormenta_gravitacional::TORMENTA_GRAVITACIONAL,
    la_defensa_imposible::LA_DEFENSA_IMPOSIBLE,
    crate::cooperacion::PROGRESION_COOPERATIVA,
];

pub fn next_mission(completed: MissionId) -> Option<MissionId> {
    CAMPAIGN_ORDER
        .windows(2)
        .find_map(|pair| (pair[0] == completed).then_some(pair[1]))
}

pub(crate) fn unlock_following_missions(session: &mut ProjectEchoSession) {
    let completed: Vec<_> = session
        .world()
        .events()
        .iter()
        .filter_map(|event| match event {
            GameplayEvent::MissionCompleted { mission, .. } => Some(*mission),
            _ => None,
        })
        .collect();

    for mission in completed {
        let Some(next) = next_mission(mission) else {
            continue;
        };
        if session
            .world()
            .mission(next)
            .is_some_and(|value| value.state() == MissionState::Locked)
        {
            let _ = session.processor.process_followup(
                &mut session.world,
                GameplayCommand::UnlockMission { mission: next },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use engine::{Duration, Vec3};
    use gameplay::domains::mission::MissionState;
    use gameplay::{
        BreakerId, BreakerState, Destination, DestinationType, DiscoveryState,
        ElectricalBreakerConfiguration, ElectricalPanelConfiguration, GameplayCommand,
        LifeSupportConfiguration, OxygenCompartmentConfiguration, StationType,
    };

    use super::*;

    #[test]
    fn campaign_order_maps_only_to_the_next_mission() {
        assert_eq!(CAMPAIGN_ORDER.len(), 5);
        assert_eq!(next_mission(CAMPAIGN_ORDER[0]), Some(CAMPAIGN_ORDER[1]));
        assert_eq!(next_mission(CAMPAIGN_ORDER[1]), Some(CAMPAIGN_ORDER[2]));
        assert_eq!(next_mission(CAMPAIGN_ORDER[2]), Some(CAMPAIGN_ORDER[3]));
        assert_eq!(next_mission(CAMPAIGN_ORDER[3]), Some(CAMPAIGN_ORDER[4]));
        assert_eq!(next_mission(CAMPAIGN_ORDER[4]), None);
    }

    #[test]
    fn normal_session_registers_all_campaign_missions_once_with_initial_states() {
        let session = ProjectEchoSession::new();

        for (index, mission) in CAMPAIGN_ORDER.iter().enumerate() {
            let progress = session
                .mission_progress(*mission)
                .expect("cada misión de campaña debe estar registrada");
            assert_eq!(progress.state, MissionState::Locked);
            assert_eq!(progress.required_completed, 0);
            assert!(progress.required_total > 0);
            if index > 0 {
                assert_eq!(progress.state, MissionState::Locked);
            }
        }
    }

    #[test]
    fn cooperative_content_is_prepared_only_after_accepted_start() {
        let mut session = ProjectEchoSession::new();
        assert!(session
            .world()
            .destination(crate::cooperacion::SIGNAL_DESTINATION)
            .is_none());
        assert_eq!(
            session
                .world()
                .combatants()
                .filter(|value| {
                    !value.is_player
                        && value.active
                        && value.archetype == Some(gameplay::EnemyArchetype::Shooter)
                })
                .count(),
            0
        );

        let unlock = session.update([GameplayCommand::UnlockMission {
            mission: crate::cooperacion::PROGRESION_COOPERATIVA,
        }]);
        assert!(matches!(
            unlock[0].status,
            gameplay::CommandStatus::Success { changed: true }
        ));
        assert!(session
            .world()
            .destination(crate::cooperacion::SIGNAL_DESTINATION)
            .is_none());

        let start = session.update([GameplayCommand::StartMission {
            mission: crate::cooperacion::PROGRESION_COOPERATIVA,
        }]);
        assert!(matches!(
            start[0].status,
            gameplay::CommandStatus::Success { changed: true }
        ));
        assert!(session
            .world()
            .destination(crate::cooperacion::SIGNAL_DESTINATION)
            .is_some());
        assert_eq!(
            session
                .world()
                .combatants()
                .filter(|value| {
                    !value.is_player
                        && value.active
                        && value.archetype == Some(gameplay::EnemyArchetype::Shooter)
                })
                .count(),
            1
        );

        let repeated = session.update([GameplayCommand::StartMission {
            mission: crate::cooperacion::PROGRESION_COOPERATIVA,
        }]);
        assert!(matches!(
            repeated[0].status,
            gameplay::CommandStatus::Rejected { .. }
        ));
        assert_eq!(
            session
                .world()
                .combatants()
                .filter(|value| {
                    !value.is_player
                        && value.active
                        && value.archetype == Some(gameplay::EnemyArchetype::Shooter)
                })
                .count(),
            1
        );
    }

    #[test]
    fn incompatible_cooperative_destination_reports_preparation_failure() {
        let mut session = ProjectEchoSession::new();
        session
            .register_destination(Destination {
                id: crate::cooperacion::SIGNAL_DESTINATION,
                destination_type: DestinationType::Planet,
                position: Vec3::ZERO,
                travel_duration: Duration(1),
                fuel_cost: 1,
                discovery: DiscoveryState::Unknown,
            })
            .expect("el destino incompatible debe poder registrarse como fixture");
        session.update([GameplayCommand::UnlockMission {
            mission: crate::cooperacion::PROGRESION_COOPERATIVA,
        }]);
        let result = session.update([GameplayCommand::StartMission {
            mission: crate::cooperacion::PROGRESION_COOPERATIVA,
        }]);
        assert!(matches!(
            result[0].status,
            gameplay::CommandStatus::Success { changed: true }
        ));
        assert!(matches!(
            result[1].status,
            gameplay::CommandStatus::Rejected {
                reason: gameplay::CommandRejection::MissionContentUnavailable
            }
        ));
        assert_eq!(
            session
                .world()
                .combatants()
                .filter(|value| {
                    !value.is_player
                        && value.active
                        && value.archetype == Some(gameplay::EnemyArchetype::Shooter)
                })
                .count(),
            0
        );
        assert_eq!(
            session
                .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
                .unwrap()
                .state,
            MissionState::Active
        );
        let retry = session.update([GameplayCommand::RetryCooperativeMissionContent]);
        let retry = &retry[0];
        assert!(matches!(
            retry.status,
            gameplay::CommandStatus::Rejected {
                reason: gameplay::CommandRejection::MissionContentUnavailable
            }
        ));
    }

    #[test]
    fn failed_cooperative_preparation_can_be_retried_without_resetting_mission() {
        let mut session = ProjectEchoSession::new();
        let player = session.ids.first_player;
        session.update([GameplayCommand::UnlockMission {
            mission: crate::cooperacion::PROGRESION_COOPERATIVA,
        }]);
        session.update([GameplayCommand::SetPlayerActive {
            player,
            active: false,
        }]);

        let start = session.update([GameplayCommand::StartMission {
            mission: crate::cooperacion::PROGRESION_COOPERATIVA,
        }]);
        assert!(matches!(
            start.last().map(|value| value.status),
            Some(gameplay::CommandStatus::Rejected {
                reason: gameplay::CommandRejection::MissionContentUnavailable
            })
        ));
        assert_eq!(
            session
                .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
                .unwrap()
                .state,
            MissionState::Active
        );
        assert!(session
            .world()
            .destination(crate::cooperacion::SIGNAL_DESTINATION)
            .is_none());

        session.update([GameplayCommand::SetPlayerActive {
            player,
            active: true,
        }]);
        let retry = session.update([GameplayCommand::RetryCooperativeMissionContent]);
        let retry = &retry[0];
        assert!(matches!(
            retry.status,
            gameplay::CommandStatus::Success { changed: true }
        ));
        assert_eq!(
            session
                .mission_progress(crate::cooperacion::PROGRESION_COOPERATIVA)
                .unwrap()
                .state,
            MissionState::Active
        );
        assert!(session
            .world()
            .destination(crate::cooperacion::SIGNAL_DESTINATION)
            .is_some());
        assert_eq!(
            session
                .world()
                .combatants()
                .filter(|value| {
                    !value.is_player
                        && value.active
                        && value.archetype == Some(gameplay::EnemyArchetype::Shooter)
                })
                .count(),
            1
        );
    }

    #[test]
    fn cooperative_content_retry_is_idempotent_and_unavailable_missions_are_not_repaired() {
        let mut session = ProjectEchoSession::new();
        session.update([GameplayCommand::UnlockMission {
            mission: crate::cooperacion::PROGRESION_COOPERATIVA,
        }]);
        session.update([GameplayCommand::StartMission {
            mission: crate::cooperacion::PROGRESION_COOPERATIVA,
        }]);
        let before = session
            .world()
            .combatants()
            .filter(|value| {
                !value.is_player
                    && value.active
                    && value.archetype == Some(gameplay::EnemyArchetype::Shooter)
            })
            .count();
        let retry = session.update([GameplayCommand::RetryCooperativeMissionContent]);
        let retry = &retry[0];
        assert!(matches!(
            retry.status,
            gameplay::CommandStatus::Success { changed: true }
        ));
        assert_eq!(
            session
                .world()
                .combatants()
                .filter(|value| {
                    !value.is_player
                        && value.active
                        && value.archetype == Some(gameplay::EnemyArchetype::Shooter)
                })
                .count(),
            before
        );

        let mut unavailable = ProjectEchoSession::new();
        let no_repair = unavailable.update([GameplayCommand::RetryCooperativeMissionContent]);
        let no_repair = &no_repair[0];
        assert!(matches!(
            no_repair.status,
            gameplay::CommandStatus::Rejected {
                reason: gameplay::CommandRejection::InvalidMissionTransition
            }
        ));
        assert!(unavailable
            .world()
            .destination(crate::cooperacion::SIGNAL_DESTINATION)
            .is_none());
    }

    #[test]
    fn completing_first_mission_unlocks_only_its_successor_and_persists() {
        let mut session = ProjectEchoSession::new();
        let player = session.ids.first_player;
        let station = session
            .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
            .expect("station should register");
        let console = session
            .create_ship_status_console(Vec3::ZERO, 2.0)
            .expect("status console should register");
        let electrical = session
            .create_electrical_panel(
                Vec3::ZERO,
                2.0,
                ElectricalPanelConfiguration::new(vec![ElectricalBreakerConfiguration {
                    id: BreakerId::new(1),
                    initial_state: BreakerState::Off,
                    target_state: BreakerState::On,
                }]),
            )
            .expect("electrical panel should register");
        let life_support = session
            .create_life_support_panel(
                Vec3::ZERO,
                2.0,
                LifeSupportConfiguration::new(
                    5,
                    vec![
                        OxygenCompartmentConfiguration {
                            id: gameplay::CompartmentId::new(1),
                            initial_allocation: 0,
                            minimum_allocation: 3,
                            maximum_allocation: 5,
                        },
                        OxygenCompartmentConfiguration {
                            id: gameplay::CompartmentId::new(2),
                            initial_allocation: 0,
                            minimum_allocation: 2,
                            maximum_allocation: 5,
                        },
                    ],
                ),
            )
            .expect("life support panel should register");
        let door = session
            .create_bridge_door(Vec3::ZERO, 2.0)
            .expect("bridge door should register");

        let run = |session: &mut ProjectEchoSession, command| {
            let result = session.update([command]);
            assert!(matches!(
                result.first().map(|value| &value.status),
                Some(gameplay::CommandStatus::Success { changed: true })
            ));
        };
        run(
            &mut session,
            GameplayCommand::UnlockMission {
                mission: el_despertar::EL_DESPERTAR,
            },
        );
        run(
            &mut session,
            GameplayCommand::StartMission {
                mission: el_despertar::EL_DESPERTAR,
            },
        );
        run(
            &mut session,
            GameplayCommand::Interact {
                player,
                target: console,
            },
        );
        run(
            &mut session,
            GameplayCommand::Interact {
                player,
                target: electrical,
            },
        );
        run(
            &mut session,
            GameplayCommand::SetElectricalBreakerState {
                player,
                panel: electrical,
                breaker: BreakerId::new(1),
                state: BreakerState::On,
            },
        );
        run(
            &mut session,
            GameplayCommand::Interact {
                player,
                target: life_support,
            },
        );
        run(
            &mut session,
            GameplayCommand::SetOxygenAllocation {
                player,
                panel: life_support,
                compartment: gameplay::CompartmentId::new(1),
                allocation: 3,
            },
        );
        run(
            &mut session,
            GameplayCommand::SetOxygenAllocation {
                player,
                panel: life_support,
                compartment: gameplay::CompartmentId::new(2),
                allocation: 2,
            },
        );
        run(&mut session, GameplayCommand::UnlockDoor { door });
        run(
            &mut session,
            GameplayCommand::Interact {
                player,
                target: door,
            },
        );
        run(
            &mut session,
            GameplayCommand::Interact {
                player,
                target: station,
            },
        );

        assert_eq!(
            session
                .mission_progress(el_despertar::EL_DESPERTAR)
                .unwrap()
                .state,
            MissionState::Completed
        );
        assert_eq!(
            session
                .mission_progress(ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                .unwrap()
                .state,
            MissionState::Available
        );
        let guidance = session
            .world()
            .guidance_for_player(player)
            .expect("guidance should be available");
        assert!(guidance.objectives.iter().any(|objective| {
            objective.mission == ecos_anomalia::ECOS_DE_LA_ANOMALIA
                && objective.recommended_action
                    == Some(gameplay::GuidanceAction::StartMission(
                        ecos_anomalia::ECOS_DE_LA_ANOMALIA,
                    ))
        }));
        for mission in CAMPAIGN_ORDER.iter().skip(2) {
            assert_eq!(
                session.mission_progress(*mission).unwrap().state,
                MissionState::Locked
            );
        }

        let before_repeat = session.save_state().serialize();
        unlock_following_missions(&mut session);
        assert_eq!(session.save_state().serialize(), before_repeat);
        let save = session.save_state();
        let mut restored = ProjectEchoSession::new();
        restored
            .restore_state(save)
            .expect("campaign state should restore");
        assert_eq!(
            restored
                .mission_progress(ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                .unwrap()
                .state,
            MissionState::Available
        );
        assert_ne!(
            restored
                .mission_progress(ecos_anomalia::ECOS_DE_LA_ANOMALIA)
                .unwrap()
                .state,
            MissionState::Active
        );
    }
}