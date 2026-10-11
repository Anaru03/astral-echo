use engine::Vec3;
use game::{el_despertar, ProjectEchoSession};
use gameplay::domains::{door::DoorState, mission::MissionState};
use gameplay::{
    BreakerId, BreakerState, ElectricalBreakerConfiguration, ElectricalPanelConfiguration,
    GameplayCommand, LifeSupportConfiguration, MissionProgress, OxygenCompartmentConfiguration,
    StationState, StationType,
};

fn run_command(
    session: &mut ProjectEchoSession,
    label: &str,
    command: GameplayCommand,
) -> Result<(), String> {
    let result = session
        .update([command])
        .into_iter()
        .next()
        .ok_or_else(|| format!("{label}: no se devolvió resultado"))?;
    match result.status {
        gameplay::CommandStatus::Success { changed: true } => Ok(()),
        status => Err(format!("{label}: resultado inesperado: {status:?}")),
    }
}

fn required_completed(session: &ProjectEchoSession) -> Result<MissionProgress, String> {
    let progress = session
        .mission_progress(el_despertar::EL_DESPERTAR)
        .ok_or_else(|| "no existe la misión El despertar".to_owned())?;
    if progress.state != MissionState::Completed
        || progress.required_completed != progress.required_total
    {
        return Err(format!("la misión no terminó: {progress:?}"));
    }
    Ok(progress)
}

fn main() {
    if let Err(error) = run_scenario() {
        eprintln!("ESCENARIO FALLIDO: {error}");
        std::process::exit(1);
    }
}

fn run_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let position = Vec3::ZERO;

    let initial = session
        .mission_progress(el_despertar::EL_DESPERTAR)
        .ok_or_else(|| "no existe la misión inicial".to_owned())?;
    if initial.state != MissionState::Locked {
        return Err(format!("la misión no comenzó bloqueada: {initial:?}"));
    }

    let status_console = session
        .create_ship_status_console(position, 2.0)
        .map_err(|error| format!("crear consola de estado: {error:?}"))?;
    let electrical_panel = session
        .create_electrical_panel(
            position,
            2.0,
            ElectricalPanelConfiguration::new(vec![ElectricalBreakerConfiguration {
                id: BreakerId::new(1),
                initial_state: BreakerState::Off,
                target_state: BreakerState::On,
            }]),
        )
        .map_err(|error| format!("crear panel eléctrico: {error:?}"))?;
    let life_support_panel = session
        .create_life_support_panel(
            position,
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
        .map_err(|error| format!("crear panel de soporte vital: {error:?}"))?;
    let bridge = session
        .create_bridge_door(position, 2.0)
        .map_err(|error| format!("crear puerta del puente: {error:?}"))?;
    let pilot_station = session
        .create_station(position, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estación de pilotaje: {error:?}"))?;

    run_command(
        &mut session,
        "desbloquear misión",
        GameplayCommand::UnlockMission {
            mission: el_despertar::EL_DESPERTAR,
        },
    )?;
    run_command(
        &mut session,
        "iniciar misión",
        GameplayCommand::StartMission {
            mission: el_despertar::EL_DESPERTAR,
        },
    )?;
    run_command(
        &mut session,
        "evaluar estado de la nave",
        GameplayCommand::Interact {
            player,
            target: status_console,
        },
    )?;
    run_command(
        &mut session,
        "iniciar interacción eléctrica",
        GameplayCommand::Interact {
            player,
            target: electrical_panel,
        },
    )?;
    run_command(
        &mut session,
        "resolver energía",
        GameplayCommand::SetElectricalBreakerState {
            player,
            panel: electrical_panel,
            breaker: BreakerId::new(1),
            state: BreakerState::On,
        },
    )?;
    run_command(
        &mut session,
        "iniciar interacción de soporte vital",
        GameplayCommand::Interact {
            player,
            target: life_support_panel,
        },
    )?;
    run_command(
        &mut session,
        "asignar oxígeno al compartimento 1",
        GameplayCommand::SetOxygenAllocation {
            player,
            panel: life_support_panel,
            compartment: gameplay::CompartmentId::new(1),
            allocation: 3,
        },
    )?;
    run_command(
        &mut session,
        "asignar oxígeno al compartimento 2",
        GameplayCommand::SetOxygenAllocation {
            player,
            panel: life_support_panel,
            compartment: gameplay::CompartmentId::new(2),
            allocation: 2,
        },
    )?;
    run_command(
        &mut session,
        "desbloquear puente",
        GameplayCommand::UnlockDoor { door: bridge },
    )?;
    run_command(
        &mut session,
        "abrir puente",
        GameplayCommand::Interact {
            player,
            target: bridge,
        },
    )?;
    run_command(
        &mut session,
        "ocupar estación de pilotaje",
        GameplayCommand::Interact {
            player,
            target: pilot_station,
        },
    )?;

    if !session
        .world()
        .electrical_panel(electrical_panel)
        .is_some_and(|panel| panel.is_repaired())
    {
        return Err("el panel eléctrico no quedó reparado".to_owned());
    }
    if !session
        .world()
        .life_support_panel(life_support_panel)
        .is_some_and(|panel| panel.is_repaired())
    {
        return Err("el soporte vital no quedó reparado".to_owned());
    }
    if session
        .world()
        .door(bridge)
        .is_none_or(|door| door.state != DoorState::Open)
    {
        return Err("la puerta del puente no quedó abierta".to_owned());
    }
    if session.world().station_state(pilot_station) != Some(StationState::Occupied) {
        return Err("la estación de pilotaje no quedó ocupada".to_owned());
    }
    let progress = required_completed(&session)?;
    println!(
        "ESCENARIO COMPLETADO: misión={:?}, obligatorios={}/{}, jugador={}, nave={}",
        progress.state,
        progress.required_completed,
        progress.required_total,
        player,
        session.ids.first_ship
    );
    Ok(())
}