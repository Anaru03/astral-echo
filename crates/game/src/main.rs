use engine::{Duration, EntityId, Vec3};
use game::{
    cooperacion, ecos_anomalia, el_despertar, la_defensa_imposible, tormenta_gravitacional,
    ProjectEchoSession,
};
use gameplay::domains::{door::DoorState, mission::MissionState, objective::ObjectiveState};
use gameplay::types::ItemId;
use gameplay::{
    AttackType, BreakerId, BreakerState, CollectibleError, Destination, DestinationId,
    DestinationType, DiscoveryState, ElectricalBreakerConfiguration, ElectricalPanelConfiguration,
    ElectricalPanelRegistrationError, EnemyArchetype, EnemyConfiguration, EnvironmentalHazard,
    CommandStatus, EncounterId, GameplayCommand, GameplayEvent, HazardId, LifeSupportConfiguration,
    LifeSupportRegistrationError, MissionProgress, OxygenCompartmentConfiguration,
    RewardDistributionPolicy, ShipRepairConfiguration, SignalKnowledge, StationState,
    AutopilotState, NavigationRoute, NavigationRouteState, SpatialEncounter,
    SpatialEncounterKind, SpatialEncounterState, StationType, TravelState,
    WeaponAmmunition, WeaponConfiguration,
};
use gameplay::{Derelict, DerelictId, DerelictReward, DerelictState};
use gameplay::{
    CombatZone, FunctionalWorld, ResourceSite, StorySite, TradeMarket, TradeOffer, WorldActivity,
};

fn run_guidance_progression_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player_a = session.ids.first_player;
    let player_b = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador de guía B: {error:?}"))?;
    let player_c = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador de guía C: {error:?}"))?;
    let ship = session.ids.first_ship;
    for player in [player_b, player_c] {
        session
            .world_mut()
            .initialize_inventory(player, 20)
            .map_err(|error| format!("inicializar inventario de guía: {error:?}"))?;
    }

    let initial = session
        .world()
        .guidance_for_player(player_a)
        .map_err(|error| format!("consultar guía inicial: {error:?}"))?;
    if initial.player != player_a
        || initial.objectives.is_empty()
        || initial.objectives.iter().all(|objective| {
            objective.status != gameplay::GuidanceStatus::Locked
        })
        || !initial.objectives.iter().any(|objective| {
            objective.missing_requirements.iter().any(|requirement| {
                matches!(requirement, gameplay::GuidanceRequirement::KnowledgeAtLeast { .. })
            })
        })
        || !initial.objectives.iter().any(|objective| {
            objective.missing_requirements.iter().any(|requirement| {
                matches!(requirement, gameplay::GuidanceRequirement::EnemyDefeated(_))
            })
        })
        || !initial.objectives.iter().any(|objective| {
            objective.missing_requirements.iter().any(|requirement| {
                matches!(requirement, gameplay::GuidanceRequirement::ResourceObtained { .. })
            })
        })
    {
        return Err("la guía inicial no reflejó requisitos reales de conocimiento, combate y recursos".to_owned());
    }

    run_command(
        &mut session,
        "desbloquear misión para guía",
        GameplayCommand::UnlockMission {
            mission: el_despertar::EL_DESPERTAR,
        },
    )?;
    let available = session
        .world()
        .guidance_for_player(player_b)
        .map_err(|error| format!("consultar guía disponible: {error:?}"))?;
    if available.player != player_b
        || !available.objectives.iter().any(|objective| {
            objective.status == gameplay::GuidanceStatus::Available
                && objective.recommended_action
                    == Some(gameplay::GuidanceAction::StartMission(
                        el_despertar::EL_DESPERTAR,
                    ))
        })
    {
        return Err("la guía no recomendó iniciar la misión disponible".to_owned());
    }

    run_command(
        &mut session,
        "iniciar misión para guía",
        GameplayCommand::StartMission {
            mission: el_despertar::EL_DESPERTAR,
        },
    )?;
    let active = session
        .world()
        .guidance_for_player(player_c)
        .map_err(|error| format!("consultar guía activa: {error:?}"))?;
    if active.objectives.iter().all(|objective| {
        objective.status != gameplay::GuidanceStatus::Active
            && objective.status != gameplay::GuidanceStatus::Available
    }) {
        return Err("la guía activa no expuso un objetivo ejecutable".to_owned());
    }
    let before = (
        session.world().time(),
        session.world().events().len(),
        session.world().player_health(player_a),
        session.world().inventory(player_a).map(|inventory| inventory.used_capacity()),
    );
    let repeated = session
        .world()
        .guidance_for_player(player_a)
        .map_err(|error| format!("consultar guía repetida: {error:?}"))?;
    let after = (
        session.world().time(),
        session.world().events().len(),
        session.world().player_health(player_a),
        session.world().inventory(player_a).map(|inventory| inventory.used_capacity()),
    );
    let active_for_a = session
        .world()
        .guidance_for_player(player_a)
        .map_err(|error| format!("consultar guía activa para A: {error:?}"))?;
    if before != after || repeated != active_for_a {
        return Err("consultar la guía modificó el mundo o no fue determinista".to_owned());
    }

    let guidance_destination = ecos_anomalia::SIGNAL_DESTINATION;
    session
        .register_destination(Destination {
            id: guidance_destination,
            destination_type: DestinationType::SpacePoint,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Unknown,
        })
        .map_err(|error| format!("registrar destino de guía: {error:?}"))?;
    run_command(
        &mut session,
        "registrar conocimiento de guía",
        GameplayCommand::RegisterKnowledge {
            target: gameplay::KnowledgeTarget::Destination(guidance_destination),
        },
    )?;
    run_command(
        &mut session,
        "escanear señal de guía",
        GameplayCommand::ScanSignals {
            player: player_a,
            ship,
        },
    )?;
    run_command(
        &mut session,
        "identificar señal de guía",
        GameplayCommand::IdentifyKnowledge {
            target: gameplay::KnowledgeTarget::Destination(guidance_destination),
        },
    )?;
    let guided_collectible = session
        .world_mut()
        .create_collectible(ItemId(3), 1, Vec3::ZERO, 2.0)
        .map_err(|error| format!("crear objeto para guía: {error:?}"))?;
    run_command(
        &mut session,
        "recoger objeto de guía",
        GameplayCommand::CollectItem {
            player: player_b,
            collectible: guided_collectible,
            amount: 1,
        },
    )?;
    let progressed = session
        .world()
        .guidance_for_player(player_c)
        .map_err(|error| format!("consultar guía progresada: {error:?}"))?;
    if progressed.objectives.iter().any(|objective| {
        objective.missing_requirements.iter().any(|requirement| {
            matches!(
                requirement,
                gameplay::GuidanceRequirement::KnowledgeAtLeast {
                    target: gameplay::KnowledgeTarget::Destination(destination),
                    state: gameplay::KnowledgeState::Identified,
                } if *destination == guidance_destination
            )
        })
    }) || progressed.objectives.iter().any(|objective| {
        objective.missing_requirements.iter().any(|requirement| {
            matches!(
                requirement,
                gameplay::GuidanceRequirement::ResourceObtained {
                    player: owner,
                    item: ItemId(3),
                    amount: 1,
                } if *owner == player_c
            )
        })
    }) {
        return Err("la guía no reflejó el conocimiento y recurso obtenidos".to_owned());
    }
    if [player_a, player_b, player_c]
        .iter()
        .any(|player| session.world().guidance_for_player(*player).is_err())
    {
        return Err("la guía no admite los tres jugadores locales activos".to_owned());
    }
    println!(
        "GUIA DE PROGRESION COMPLETADA: jugadores=3, objetivos={}, actual={:?}",
        active.objectives.len(),
        active.current_objective
    );
    Ok(())
}

fn run_group_scaling_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player_a = session.ids.first_player;
    let player_b = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador B para escalado: {error:?}"))?;
    let player_c = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador C para escalado: {error:?}"))?;
    for player in [player_b, player_c] {
        session
            .world_mut()
            .initialize_inventory(player, 20)
            .map_err(|error| format!("inicializar inventario de escalado: {error:?}"))?;
    }

    let solo = session.world().current_group_scaling();
    if solo.group_size != gameplay::GroupSize::Trio {
        return Err("la sesión no reflejó inicialmente el trío activo".to_owned());
    }
    run_command(
        &mut session,
        "desactivar jugador C para solo",
        GameplayCommand::SetPlayerActive {
            player: player_c,
            active: false,
        },
    )?;
    run_command(
        &mut session,
        "desactivar jugador B para solo",
        GameplayCommand::SetPlayerActive {
            player: player_b,
            active: false,
        },
    )?;
    let solo = session.world().current_group_scaling();
    if solo.group_size != gameplay::GroupSize::Solo
        || solo.enemy_health_percent != 100
        || solo.enemy_damage_percent != 100
    {
        return Err("el escalado solo no conserva el nivel base".to_owned());
    }
    run_command(
        &mut session,
        "reactivar jugador B para duo",
        GameplayCommand::SetPlayerActive {
            player: player_b,
            active: true,
        },
    )?;
    let duo = session.world().current_group_scaling();
    if duo.group_size != gameplay::GroupSize::Duo
        || duo.enemy_health_percent <= solo.enemy_health_percent
        || duo.enemy_damage_percent <= solo.enemy_damage_percent
    {
        return Err("el escalado duo no fue moderado y creciente".to_owned());
    }
    run_command(
        &mut session,
        "reactivar jugador C para trio",
        GameplayCommand::SetPlayerActive {
            player: player_c,
            active: true,
        },
    )?;
    let trio = session.world().current_group_scaling();
    if trio.group_size != gameplay::GroupSize::Trio
        || trio.enemy_health_percent <= duo.enemy_health_percent
        || trio.enemy_damage_percent <= duo.enemy_damage_percent
        || trio.enemy_health_percent > solo.enemy_health_percent * 3
        || trio.enemy_damage_percent > solo.enemy_damage_percent * 3
    {
        return Err("el escalado trio excedio los limites".to_owned());
    }
    for _ in 0..3 {
        if session.world().current_group_scaling() != trio {
            return Err("consultar el escalado no fue determinista".to_owned());
        }
    }
    run_command(
        &mut session,
        "desactivar jugador A para grupo vacio parcial",
        GameplayCommand::SetPlayerActive {
            player: player_a,
            active: false,
        },
    )?;
    run_command(
        &mut session,
        "desactivar jugador B para grupo vacio parcial",
        GameplayCommand::SetPlayerActive {
            player: player_b,
            active: false,
        },
    )?;
    run_command(
        &mut session,
        "desactivar jugador C para grupo vacio",
        GameplayCommand::SetPlayerActive {
            player: player_c,
            active: false,
        },
    )?;
    if session.world().current_group_scaling().group_size != gameplay::GroupSize::Empty
        || session
            .world()
            .scaling_for_player(player_a)
            .is_ok()
        || gameplay::GameplayScaling::for_group_size(gameplay::GroupSize::Empty)
            .scale_enemy_configuration(EnemyConfiguration::for_archetype(
                EnemyArchetype::Shooter,
                Vec3::ZERO,
            ))
            != Err(gameplay::ScalingError::NoActivePlayers)
    {
        return Err("el grupo vacio no fue rechazado de forma segura".to_owned());
    }
    for player in [player_a, player_b, player_c] {
        run_command(
            &mut session,
            "reactivar jugador tras grupo vacio",
            GameplayCommand::SetPlayerActive {
                player,
                active: true,
            },
        )?;
    }
    if session.world().current_group_scaling().group_size != gameplay::GroupSize::Trio
        || session
            .world()
            .scaling_for_player(EntityId::new(999_999))
            .is_ok()
        || gameplay::GameplayScaling::for_group_size(gameplay::GroupSize::Trio)
            .scale_amount(u32::MAX, 100)
            != Err(gameplay::ScalingError::ArithmeticOverflow)
    {
        return Err("las validaciones de identidad o overflow fallaron".to_owned());
    }

    let destination = DestinationId::new(5601);
    session
        .register_destination(Destination {
            id: destination,
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        })
        .map_err(|error| format!("registrar destino de escalado: {error:?}"))?;
    let base_configuration = EnemyConfiguration {
        archetype: EnemyArchetype::Shooter,
        position: Vec3::ZERO,
        maximum_health: 20,
        weapon: WeaponConfiguration::new(AttackType::Energy, 5, 20, Duration(1_000)),
        shield: None,
        mobility: 1,
    };
    run_command(
        &mut session,
        "registrar mundo hostil escalable",
        GameplayCommand::RegisterFunctionalWorld {
            world: FunctionalWorld::new(
                destination,
                2.0,
                vec![WorldActivity::Combat(CombatZone {
                    configuration: base_configuration,
                    active_enemy: None,
                    resolved: false,
                })],
            ),
        },
    )?;
    run_command(
        &mut session,
        "activar encuentro hostil escalado",
        GameplayCommand::ActivateWorldCombat {
            player: player_c,
            destination,
        },
    )?;
    let enemy = session
        .world()
        .functional_world(destination)
        .and_then(|functional_world| {
            functional_world.activities.iter().find_map(|activity| match activity {
                WorldActivity::Combat(zone) => zone.active_enemy,
                _ => None,
            })
        })
        .ok_or_else(|| "el encuentro escalado no creo enemigo".to_owned())?;
    let scaled_enemy = session
        .world()
        .combatant(enemy)
        .ok_or_else(|| "falta combatiente escalado".to_owned())?;
    if scaled_enemy.maximum_health != 30 || scaled_enemy.weapon.damage != 6 {
        return Err(format!(
            "la configuracion efectiva no uso el escalado trio: salud={}, dano={}",
            scaled_enemy.maximum_health, scaled_enemy.weapon.damage
        ));
    }
    run_command(
        &mut session,
        "equipar arma contra encuentro escalado",
        GameplayCommand::EquipPlayerWeapon {
            player: player_c,
            weapon: WeaponConfiguration::new(AttackType::Kinetic, 40, 20, Duration(1)),
        },
    )?;
    let collectibles_before = session.world().collectibles().filter(|value| value.active).count();
    run_command(
        &mut session,
        "derrotar enemigo escalado",
        GameplayCommand::AttackEnemy {
            player: player_c,
            enemy,
        },
    )?;
    let collectibles_after = session.world().collectibles().filter(|value| value.active).count();
    if session.world().combatant(enemy).is_none_or(|value| value.active || !value.is_defeated())
        || collectibles_after != collectibles_before + 1
    {
        return Err("el combate escalado no produjo derrota y recompensa unica".to_owned());
    }
    let repeated_attack = session.update([GameplayCommand::AttackEnemy {
        player: player_c,
        enemy,
    }]);
    if repeated_attack.first().is_none_or(|result| {
        !matches!(result.status, CommandStatus::Rejected { .. })
    }) || session.world().collectibles().filter(|value| value.active).count() != collectibles_after
    {
        return Err("el enemigo escalado o su recompensa se duplicaron".to_owned());
    }
    let reward = session
        .world()
        .collectibles()
        .find(|value| value.active)
        .map(|value| value.entity)
        .ok_or_else(|| "falta recompensa del encuentro escalado".to_owned())?;
    run_command(
        &mut session,
        "recoger recompensa del encuentro escalado",
        GameplayCommand::CollectItem {
            player: player_c,
            collectible: reward,
            amount: 1,
        },
    )?;

    let derelict_id = DerelictId::new(5602);
    run_command(
        &mut session,
        "registrar derelict escalable",
        GameplayCommand::RegisterDerelict {
            derelict: Derelict::new(derelict_id, Vec3::ZERO, 2.0)
                .with_enemy(base_configuration)
                .with_rewards(
                    vec![DerelictReward {
                        item: ItemId(1),
                        amount: 1,
                    }],
                    RewardDistributionPolicy::Shared,
                ),
        },
    )?;
    run_command(
        &mut session,
        "descubrir derelict escalable",
        GameplayCommand::DiscoverDerelict {
            player: player_a,
            derelict: derelict_id,
        },
    )?;
    run_command(
        &mut session,
        "activar derelict escalable",
        GameplayCommand::ExploreDerelict {
            player: player_a,
            derelict: derelict_id,
        },
    )?;
    let derelict_enemy = session
        .world()
        .derelict(derelict_id)
        .and_then(|value| value.spawned_enemy)
        .ok_or_else(|| "el derelict escalable no creo su enemigo".to_owned())?;
    if session
        .world()
        .combatant(derelict_enemy)
        .is_none_or(|value| value.maximum_health != 30)
    {
        return Err("el derelict no uso la configuracion escalada".to_owned());
    }
    run_command(
        &mut session,
        "equipar arma contra derelict escalado",
        GameplayCommand::EquipPlayerWeapon {
            player: player_a,
            weapon: WeaponConfiguration::new(AttackType::Kinetic, 40, 20, Duration(1)),
        },
    )?;
    run_command(
        &mut session,
        "derrotar enemigo del derelict escalado",
        GameplayCommand::AttackEnemy {
            player: player_a,
            enemy: derelict_enemy,
        },
    )?;
    let derelict_reward = session
        .world()
        .derelict(derelict_id)
        .and_then(|value| value.reward_collectibles.first().copied())
        .ok_or_else(|| "el derelict escalado no genero recompensa".to_owned())?;
    run_command(
        &mut session,
        "recoger recompensa del derelict escalado",
        GameplayCommand::CollectItem {
            player: player_b,
            collectible: derelict_reward,
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "resolver derelict escalado",
        GameplayCommand::ResolveDerelict {
            player: player_a,
            derelict: derelict_id,
        },
    )?;
    let repeated_explore = session.update([GameplayCommand::ExploreDerelict {
        player: player_b,
        derelict: derelict_id,
    }]);
    if repeated_explore.first().is_none_or(|result| {
        result.status
            != (CommandStatus::Rejected {
                reason: gameplay::CommandRejection::DerelictAlreadyResolved,
            })
    }) {
        return Err("el derelict escalado no fue idempotente".to_owned());
    }
    println!(
        "ESCALADO DE GRUPO COMPLETADO: solo=100%, duo={}%, trio={}%, enemigo_salud=30, derelict_resuelto=true",
        duo.enemy_health_percent,
        trio.enemy_health_percent
    );
    Ok(())
}

fn run_atomicity_regressions() -> Result<(), String> {
    let electrical_configuration = || {
        ElectricalPanelConfiguration::new(vec![ElectricalBreakerConfiguration {
            id: BreakerId::new(1),
            initial_state: BreakerState::Off,
            target_state: BreakerState::On,
        }])
    };
    let mut electrical_control = ProjectEchoSession::new();
    let expected_electrical = electrical_control
        .create_electrical_panel(Vec3::ZERO, 2.0, electrical_configuration())
        .map_err(|error| format!("crear panel eléctrico de control: {error:?}"))?;
    let mut electrical_session = ProjectEchoSession::new();
    let electrical_active = electrical_session.world().entities().active_count();
    let electrical_registered = electrical_session.world().electrical_panels().count();
    let electrical_events = electrical_session.world().events().len();
    let electrical_error = electrical_session
        .create_electrical_panel(
            Vec3::ZERO,
            2.0,
            ElectricalPanelConfiguration::new(Vec::new()),
        )
        .expect_err("un panel eléctrico vacío debe rechazarse");
    if electrical_error != ElectricalPanelRegistrationError::EmptyConfiguration {
        return Err(format!("error eléctrico inesperado: {electrical_error:?}"));
    }
    if electrical_session.world().entities().active_count() != electrical_active
        || electrical_session.world().electrical_panels().count() != electrical_registered
        || electrical_session.world().events().len() != electrical_events
    {
        return Err("el rechazo eléctrico modificó el mundo".to_owned());
    }
    let actual_electrical = electrical_session
        .create_electrical_panel(Vec3::ZERO, 2.0, electrical_configuration())
        .map_err(|error| format!("crear panel eléctrico posterior: {error:?}"))?;
    if actual_electrical != expected_electrical {
        return Err("el rechazo eléctrico consumió un identificador".to_owned());
    }

    let life_support_configuration = || {
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
        )
    };
    let mut life_support_control = ProjectEchoSession::new();
    let expected_life_support = life_support_control
        .create_life_support_panel(Vec3::ZERO, 2.0, life_support_configuration())
        .map_err(|error| format!("crear soporte vital de control: {error:?}"))?;
    let mut life_support_session = ProjectEchoSession::new();
    let life_support_active = life_support_session.world().entities().active_count();
    let life_support_registered = life_support_session.world().life_support_panels().count();
    let life_support_events = life_support_session.world().events().len();
    let life_support_error = life_support_session
        .create_life_support_panel(
            Vec3::ZERO,
            2.0,
            LifeSupportConfiguration::new(5, Vec::new()),
        )
        .expect_err("un soporte vital vacío debe rechazarse");
    if life_support_error != LifeSupportRegistrationError::EmptyConfiguration {
        return Err(format!(
            "error de soporte vital inesperado: {life_support_error:?}"
        ));
    }
    if life_support_session.world().entities().active_count() != life_support_active
        || life_support_session.world().life_support_panels().count() != life_support_registered
        || life_support_session.world().events().len() != life_support_events
    {
        return Err("el rechazo de soporte vital modificó el mundo".to_owned());
    }
    let actual_life_support = life_support_session
        .create_life_support_panel(Vec3::ZERO, 2.0, life_support_configuration())
        .map_err(|error| format!("crear soporte vital posterior: {error:?}"))?;
    if actual_life_support != expected_life_support {
        return Err("el rechazo de soporte vital consumió un identificador".to_owned());
    }

    let item = ShipRepairConfiguration::default().material;
    let mut collectible_control = ProjectEchoSession::new();
    let expected_collectible = collectible_control
        .world_mut()
        .create_collectible(item, 1, Vec3::ZERO, 2.0)
        .map_err(|error| format!("crear recolectable de control: {error:?}"))?;
    let mut collectible_session = ProjectEchoSession::new();
    let collectible_active = collectible_session.world().entities().active_count();
    let collectible_registered = collectible_session.world().collectibles().count();
    let collectible_events = collectible_session.world().events().len();
    let collectible_error = collectible_session
        .world_mut()
        .create_collectible(item, 0, Vec3::ZERO, 2.0)
        .expect_err("un recolectable sin cantidad debe rechazarse");
    if collectible_error != CollectibleError::InvalidAmount {
        return Err(format!(
            "error de recolectable inesperado: {collectible_error:?}"
        ));
    }
    if collectible_session.world().entities().active_count() != collectible_active
        || collectible_session.world().collectibles().count() != collectible_registered
        || collectible_session.world().events().len() != collectible_events
    {
        return Err("el rechazo del recolectable modificó el mundo".to_owned());
    }
    let actual_collectible = collectible_session
        .world_mut()
        .create_collectible(item, 1, Vec3::ZERO, 2.0)
        .map_err(|error| format!("crear recolectable posterior: {error:?}"))?;
    if actual_collectible != expected_collectible {
        return Err("el rechazo del recolectable consumió un identificador".to_owned());
    }

    println!("REGRESIONES DE ATOMICIDAD COMPLETADAS");
    Ok(())
}

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

fn run_derelict_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player_a = session.ids.first_player;
    let player_b = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear explorador B: {error:?}"))?;
    let player_c = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear explorador C: {error:?}"))?;
    for player in [player_b, player_c] {
        session
            .world_mut()
            .initialize_inventory(player, 20)
            .map_err(|error| format!("inicializar inventario del explorador: {error:?}"))?;
    }
    let enemy_configuration = EnemyConfiguration {
        archetype: EnemyArchetype::Shooter,
        position: Vec3::ZERO,
        maximum_health: 20,
        weapon: WeaponConfiguration::new(AttackType::Energy, 1, 1, Duration(1_000)),
        shield: None,
        mobility: 1,
    };
    let hazard = gameplay::HazardId::new(950);
    let hazard_encounter = EncounterId::new(950);
    run_command(
        &mut session,
        "registrar peligro del derelicto",
        GameplayCommand::RegisterEnvironmentalHazard {
            hazard: EnvironmentalHazard::new(
                hazard,
                gameplay::EnvironmentalHazardType::DebrisField,
                Vec3::ZERO,
                2.0,
                7,
                Duration(10_000),
                true,
            ),
        },
    )?;
    run_command(
        &mut session,
        "registrar encuentro del derelicto",
        GameplayCommand::RegisterSpatialEncounter {
            encounter: SpatialEncounter::new(
                hazard_encounter,
                SpatialEncounterKind::Hazard { hazard },
                Vec3::ZERO,
                2.0,
            ),
        },
    )?;
    let derelict_id = DerelictId::new(1);
    let derelict = Derelict::new(derelict_id, Vec3::ZERO, 2.0)
        .with_encounter(hazard_encounter)
        .with_enemy(enemy_configuration)
        .with_rewards(
            vec![DerelictReward {
                item: ItemId(1),
                amount: 1,
            }],
            RewardDistributionPolicy::Shared,
        );
    let before = session
        .update([GameplayCommand::RegisterDerelict { derelict }])
        .into_iter()
        .next()
        .ok_or_else(|| "registrar derelicto no devolvio resultado".to_owned())?;
    if !matches!(before.status, CommandStatus::Success { changed: true }) {
        return Err(format!("registro de derelicto rechazado: {:?}", before.status));
    }
    if session.world().derelict(derelict_id).is_none_or(|value| {
        value.state != DerelictState::Undiscovered
    }) {
        return Err("el derelicto no comenzo desconocido".to_owned());
    }
    let rejected_access = session
        .update([GameplayCommand::ExploreDerelict {
            player: player_a,
            derelict: derelict_id,
        }])
        .into_iter()
        .next()
        .ok_or_else(|| "acceso previo no devolvio resultado".to_owned())?;
    if rejected_access.status
        != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::DerelictNotDiscovered,
        })
    {
        return Err(format!("se accedio antes de descubrir: {:?}", rejected_access.status));
    }
    run_command(
        &mut session,
        "descubrir derelicto",
        GameplayCommand::DiscoverDerelict {
            player: player_a,
            derelict: derelict_id,
        },
    )?;
    if session.world().derelict(derelict_id).is_none_or(|value| {
        value.state != DerelictState::Discovered
    }) {
        return Err("el descubrimiento del derelicto no fue persistente".to_owned());
    }
    run_command(
        &mut session,
        "explorar derelicto",
        GameplayCommand::ExploreDerelict {
            player: player_a,
            derelict: derelict_id,
        },
    )?;
    let integrity_before = session
        .world()
        .ship_integrity(session.ids.first_ship)
        .ok_or_else(|| "falta integridad antes del peligro del derelicto".to_owned())?
        .current();
    run_command(
        &mut session,
        "resolver peligro del derelicto",
        GameplayCommand::ResolveSpatialEncounter {
            player: player_a,
            encounter: hazard_encounter,
        },
    )?;
    let integrity_after = session
        .world()
        .ship_integrity(session.ids.first_ship)
        .ok_or_else(|| "falta integridad despues del peligro del derelicto".to_owned())?
        .current();
    if integrity_after >= integrity_before {
        return Err("el peligro del derelicto no dano la nave".to_owned());
    }
    let repeated_hazard = session.update([GameplayCommand::ResolveSpatialEncounter {
        player: player_a,
        encounter: hazard_encounter,
    }]);
    if repeated_hazard.first().is_none_or(|result| {
        result.status != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::SpatialEncounterAlreadyResolved,
        })
    }) {
        return Err("el peligro del derelicto pudo resolverse dos veces".to_owned());
    }
    let enemy = session
        .world()
        .derelict(derelict_id)
        .and_then(|value| value.spawned_enemy)
        .ok_or_else(|| "la exploracion no activo un enemigo real".to_owned())?;
    let before_repeat = session.world().entities().active_count();
    let repeat = session.update([GameplayCommand::ExploreDerelict {
        player: player_b,
        derelict: derelict_id,
    }]);
    if repeat.first().is_none_or(|result| {
        result.status
            != (CommandStatus::Rejected {
                reason: gameplay::CommandRejection::DerelictAlreadyExploring,
            })
    }) || session.world().entities().active_count() != before_repeat
    {
        return Err("la exploracion repetida genero contenido duplicado".to_owned());
    }
    run_command(
        &mut session,
        "equipar arma del explorador",
        GameplayCommand::EquipPlayerWeapon {
            player: player_b,
            weapon: WeaponConfiguration::new(AttackType::Kinetic, 40, 20, Duration(1_000)),
        },
    )?;
    run_command(
        &mut session,
        "derrotar enemigo del derelicto",
        GameplayCommand::AttackEnemy {
            player: player_b,
            enemy,
        },
    )?;
    if session.world().derelict(derelict_id).is_none_or(|value| {
        !value.enemy_defeated || !value.reward_generated
    }) {
        return Err("la derrota no resolvio el contenido ni genero loot".to_owned());
    }
    let reward = session
        .world()
        .derelict(derelict_id)
        .and_then(|value| value.reward_collectibles.first().copied())
        .ok_or_else(|| "el derelicto no genero un recolectable".to_owned())?;
    run_command(
        &mut session,
        "recoger loot compartido",
        GameplayCommand::CollectItem {
            player: player_c,
            collectible: reward,
            amount: 1,
        },
    )?;
    let second_collect = session.update([GameplayCommand::CollectItem {
        player: player_a,
        collectible: reward,
        amount: 1,
    }]);
    if second_collect.first().is_none_or(|result| {
        result.status != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::CollectibleUnavailable,
        })
    }) {
        return Err("el loot compartido pudo recogerse dos veces".to_owned());
    }
    run_command(
        &mut session,
        "resolver derelicto",
        GameplayCommand::ResolveDerelict {
            player: player_a,
            derelict: derelict_id,
        },
    )?;
    let resolved_repeat = session.update([GameplayCommand::ExploreDerelict {
        player: player_a,
        derelict: derelict_id,
    }]);
    if resolved_repeat.first().is_none_or(|result| {
        result.status
            != (CommandStatus::Rejected {
                reason: gameplay::CommandRejection::DerelictAlreadyResolved,
            })
    }) || session.world().derelicts().count() != 1
    {
        return Err("el derelicto resuelto no fue idempotente".to_owned());
    }
    let personal_id = DerelictId::new(2);
    run_command(
        &mut session,
        "registrar derelicto personal",
        GameplayCommand::RegisterDerelict {
            derelict: Derelict::new(personal_id, Vec3::ZERO, 2.0).with_rewards(
                vec![DerelictReward {
                    item: ItemId(3),
                    amount: 1,
                }],
                RewardDistributionPolicy::Personal,
            ),
        },
    )?;
    run_command(
        &mut session,
        "descubrir derelicto personal",
        GameplayCommand::DiscoverDerelict {
            player: player_a,
            derelict: personal_id,
        },
    )?;
    run_command(
        &mut session,
        "explorar derelicto personal",
        GameplayCommand::ExploreDerelict {
            player: player_a,
            derelict: personal_id,
        },
    )?;
    let personal_rewards: Vec<_> = session
        .world()
        .derelict(personal_id)
        .map(|value| value.reward_collectibles.clone())
        .unwrap_or_default();
    if personal_rewards.len() != 3 {
        return Err(format!("recompensas personales inesperadas: {personal_rewards:?}"));
    }
    let owned = personal_rewards
        .iter()
        .copied()
        .find(|collectible| session.world().collectible(*collectible).is_some_and(|value| value.owner == Some(player_a)))
        .ok_or_else(|| "no existe recompensa personal del jugador A".to_owned())?;
    let foreign = personal_rewards
        .iter()
        .copied()
        .find(|collectible| session.world().collectible(*collectible).is_some_and(|value| value.owner == Some(player_b)))
        .ok_or_else(|| "no existe recompensa personal del jugador B".to_owned())?;
    let foreign_result = session.update([GameplayCommand::CollectItem {
        player: player_a,
        collectible: foreign,
        amount: 1,
    }]);
    if foreign_result.first().is_none_or(|result| {
        result.status != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::CollectibleNotOwned,
        })
    }) {
        return Err("se permitio recoger loot personal ajeno".to_owned());
    }
    for (player, collectible) in [
        (player_a, owned),
        (player_b, foreign),
        (player_c, personal_rewards[2]),
    ] {
        run_command(
            &mut session,
            "recoger recompensa personal",
            GameplayCommand::CollectItem {
                player,
                collectible,
                amount: 1,
            },
        )?;
    }
    run_command(
        &mut session,
        "resolver derelicto personal",
        GameplayCommand::ResolveDerelict {
            player: player_a,
            derelict: personal_id,
        },
    )?;
    if session.world().derelict(personal_id).is_none_or(|value| value.state != DerelictState::Resolved) {
        return Err("el derelicto personal no quedo resuelto".to_owned());
    }
    println!(
        "DERELICTOS COMPLETADOS: compartido={:?}, personal=3 propietarios, jugadores={:?}",
        derelict_id, [player_a, player_b, player_c]
    );
    Ok(())
}

fn run_functional_world_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player_a = session.ids.first_player;
    let player_b = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador B para mundos: {error:?}"))?;
    let player_c = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador C para mundos: {error:?}"))?;
    for player in [player_b, player_c] {
        session
            .world_mut()
            .initialize_inventory(player, 20)
            .map_err(|error| format!("inicializar inventario para mundos: {error:?}"))?;
    }
    let destinations = [
        (2001, "recursos"),
        (2002, "comercio"),
        (2003, "combate"),
        (2004, "historia"),
    ];
    for (id, label) in destinations {
        run_command(
            &mut session,
            label,
            GameplayCommand::RegisterDestination {
                destination: Destination {
                    id: DestinationId::new(id),
                    destination_type: DestinationType::Planet,
                    position: Vec3::ZERO,
                    travel_duration: Duration(1),
                    fuel_cost: 1,
                    discovery: DiscoveryState::Discovered,
                },
            },
        )?;
    }
    let resource_destination = DestinationId::new(2001);
    let trade_destination = DestinationId::new(2002);
    let combat_destination = DestinationId::new(2003);
    let story_destination = DestinationId::new(2004);
    run_command(
        &mut session,
        "registrar mundo de recursos",
        GameplayCommand::RegisterFunctionalWorld {
            world: FunctionalWorld::new(
                resource_destination,
                2.0,
                vec![WorldActivity::Resource(ResourceSite {
                    item: ItemId(1),
                    available: 3,
                    extraction_amount: 2,
                })],
            ),
        },
    )?;
    run_command(
        &mut session,
        "registrar mundo comercial",
        GameplayCommand::RegisterFunctionalWorld {
            world: FunctionalWorld::new(
                trade_destination,
                2.0,
                vec![WorldActivity::Trade(TradeMarket {
                    offers: vec![
                        TradeOffer {
                            item: ItemId(1),
                            buy_price: 1,
                            sell_price: 1,
                            stock: 4,
                        },
                        TradeOffer {
                            item: ItemId(3),
                            buy_price: 2,
                            sell_price: 1,
                            stock: 4,
                        },
                    ],
                })],
            ),
        },
    )?;
    run_command(
        &mut session,
        "registrar mundo de combate",
        GameplayCommand::RegisterFunctionalWorld {
            world: FunctionalWorld::new(
                combat_destination,
                2.0,
                vec![WorldActivity::Combat(CombatZone {
                    configuration: EnemyConfiguration {
                        archetype: EnemyArchetype::Shooter,
                        position: Vec3::ZERO,
                        maximum_health: 20,
                        weapon: WeaponConfiguration::new(
                            AttackType::Energy,
                            1,
                            1,
                            Duration(1_000),
                        ),
                        shield: None,
                        mobility: 1,
                    },
                    active_enemy: None,
                    resolved: false,
                })],
            ),
        },
    )?;
    run_command(
        &mut session,
        "registrar mundo de historia",
        GameplayCommand::RegisterFunctionalWorld {
            world: FunctionalWorld::new(
                story_destination,
                2.0,
                vec![WorldActivity::Story(StorySite {
                    target: gameplay::KnowledgeTarget::Destination(story_destination),
                    interacted: false,
                })],
            ),
        },
    )?;
    let offer = session
        .world()
        .world_trade_offer(trade_destination, ItemId(3))
        .ok_or_else(|| "no se pudo consultar el precio del mercado".to_owned())?;
    if offer.buy_price != 2 || offer.sell_price != 1 || offer.stock != 4 {
        return Err(format!("precio o disponibilidad inesperados: {offer:?}"));
    }
    let resource_before = session
        .world()
        .inventory(player_a)
        .ok_or_else(|| "falta inventario de recursos".to_owned())?
        .quantity(ItemId(1));
    run_command(
        &mut session,
        "extraer recursos",
        GameplayCommand::ExtractWorldResource {
            player: player_a,
            destination: resource_destination,
        },
    )?;
    if session.world().inventory(player_a).is_none_or(|inventory| {
        inventory.quantity(ItemId(1)) != resource_before + 2
    }) {
        return Err("la extracción no actualizo el inventario".to_owned());
    }
    run_command(
        &mut session,
        "extraer reserva restante",
        GameplayCommand::ExtractWorldResource {
            player: player_a,
            destination: resource_destination,
        },
    )?;
    let exhausted = session.update([GameplayCommand::ExtractWorldResource {
        player: player_a,
        destination: resource_destination,
    }]);
    if exhausted.first().is_none_or(|result| {
        result.status != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::WorldResourceUnavailable,
        })
    }) {
        return Err("la extracción agotada no fue rechazada".to_owned());
    }
    let funds_before = session.world().player_currency(player_a).unwrap_or(0);
    run_command(
        &mut session,
        "preparar fondos mediante venta",
        GameplayCommand::SellWorldItem {
            player: player_a,
            destination: trade_destination,
            item: ItemId(1),
            amount: 2,
        },
    )?;
    let funds_after_sale = session.world().player_currency(player_a).unwrap_or(0);
    if funds_after_sale != funds_before + 2 {
        return Err("la venta no acredito el saldo esperado".to_owned());
    }
    run_command(
        &mut session,
        "comprar objeto existente",
        GameplayCommand::BuyWorldItem {
            player: player_a,
            destination: trade_destination,
            item: ItemId(3),
            amount: 1,
        },
    )?;
    if session.world().player_currency(player_a) != Some(0)
        || session
            .world()
            .inventory(player_a)
            .is_none_or(|inventory| inventory.quantity(ItemId(3)) != 1)
    {
        return Err("la compra no fue atomica o no actualizo el saldo".to_owned());
    }
    let no_funds_balance = session.world().player_currency(player_a);
    let no_funds_inventory = session.world().inventory(player_a).cloned();
    let no_funds = session.update([GameplayCommand::BuyWorldItem {
        player: player_a,
        destination: trade_destination,
        item: ItemId(3),
        amount: 1,
    }]);
    if no_funds.first().is_none_or(|result| {
        result.status != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::WorldInsufficientFunds,
        })
    }) {
        return Err("la compra sin fondos no fue rechazada".to_owned());
    }
    if session.world().player_currency(player_a) != no_funds_balance
        || session.world().inventory(player_a).cloned() != no_funds_inventory
    {
        return Err("la compra sin fondos dejo cambios parciales".to_owned());
    }
    run_command(
        &mut session,
        "vender objeto comprado",
        GameplayCommand::SellWorldItem {
            player: player_a,
            destination: trade_destination,
            item: ItemId(3),
            amount: 1,
        },
    )?;
    let no_inventory_balance = session.world().player_currency(player_a);
    let no_inventory_snapshot = session.world().inventory(player_a).cloned();
    let no_inventory = session.update([GameplayCommand::SellWorldItem {
        player: player_a,
        destination: trade_destination,
        item: ItemId(3),
        amount: 1,
    }]);
    if no_inventory.first().is_none_or(|result| {
        result.status != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::WorldInventoryInsufficient,
        })
    }) {
        return Err("la venta sin inventario no fue rechazada".to_owned());
    }
    if session.world().player_currency(player_a) != no_inventory_balance
        || session.world().inventory(player_a).cloned() != no_inventory_snapshot
    {
        return Err("la venta sin inventario dejo cambios parciales".to_owned());
    }
    let balance_b_before = session.world().player_currency(player_b).unwrap_or(0);
    run_command(
        &mut session,
        "dar material al jugador B",
        GameplayCommand::AddInventoryItem {
            player: player_b,
            item: ItemId(1),
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "vender desde jugador B",
        GameplayCommand::SellWorldItem {
            player: player_b,
            destination: trade_destination,
            item: ItemId(1),
            amount: 1,
        },
    )?;
    if session.world().player_currency(player_a) != Some(1)
        || session.world().player_currency(player_b) != Some(balance_b_before + 1)
        || session.world().player_currency(player_c) != Some(0)
    {
        return Err("los saldos cooperativos no son independientes".to_owned());
    }
    run_command(
        &mut session,
        "activar combate del mundo",
        GameplayCommand::ActivateWorldCombat {
            player: player_b,
            destination: combat_destination,
        },
    )?;
    let enemy = session
        .world()
        .functional_world(combat_destination)
        .and_then(|functional_world| {
            functional_world.activities.iter().find_map(|activity| match activity {
                WorldActivity::Combat(zone) => zone.active_enemy,
                _ => None,
            })
        })
        .ok_or_else(|| "el mundo de combate no creo enemigo".to_owned())?;
    let repeated_combat = session.update([GameplayCommand::ActivateWorldCombat {
        player: player_c,
        destination: combat_destination,
    }]);
    if repeated_combat.first().is_none_or(|result| {
        result.status != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::WorldCombatAlreadyActivated,
        })
    }) {
        return Err("el mundo hostil genero enemigos duplicados".to_owned());
    }
    let health_before_ai: Vec<_> = [player_a, player_b, player_c]
        .iter()
        .filter_map(|player| {
            session
                .world()
                .player_health(*player)
                .map(|health| (*player, health.0))
        })
        .collect();
    run_command(
        &mut session,
        "avanzar IA del mundo hostil",
        GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        },
    )?;
    if !health_before_ai.iter().any(|(player, health)| {
        session
            .world()
            .player_health(*player)
            .is_some_and(|current| current.0 < *health)
    }) {
        return Err("la IA del mundo hostil no produjo una accion observable".to_owned());
    }
    run_command(
        &mut session,
        "equipar arma del mundo hostil",
        GameplayCommand::EquipPlayerWeapon {
            player: player_c,
            weapon: WeaponConfiguration::new(AttackType::Kinetic, 40, 20, Duration(1_000)),
        },
    )?;
    run_command(
        &mut session,
        "derrotar enemigo del mundo hostil",
        GameplayCommand::AttackEnemy {
            player: player_c,
            enemy,
        },
    )?;
    if session
        .world()
        .combatant(enemy)
        .is_none_or(|combatant| combatant.active || combatant.current_health != 0)
    {
        return Err("el enemigo del mundo hostil no fue derrotado".to_owned());
    }
    if session
        .world()
        .collectibles()
        .all(|collectible| collectible.entity != enemy && !collectible.active)
    {
        return Err("el combate del mundo no genero una recompensa observable".to_owned());
    }
    let story_before = session.world().knowledge(gameplay::KnowledgeTarget::Destination(story_destination));
    run_command(
        &mut session,
        "interactuar mundo de historia",
        GameplayCommand::InteractStoryWorld {
            player: player_a,
            destination: story_destination,
        },
    )?;
    if session.world().knowledge(gameplay::KnowledgeTarget::Destination(story_destination)) <= story_before {
        return Err("el mundo de historia no registro conocimiento".to_owned());
    }
    let repeated_story = session.update([GameplayCommand::InteractStoryWorld {
        player: player_b,
        destination: story_destination,
    }]);
    if repeated_story.first().is_none_or(|result| {
        result.status != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::WorldStoryAlreadyInteracted,
        })
    }) {
        return Err("la historia pudo interactuarse dos veces".to_owned());
    }
    if session.world().inventory(player_a).is_none_or(|inventory| {
        inventory.quantity(ItemId(3)) != 0
    }) || session.world().inventory(player_b).is_none_or(|inventory| {
        inventory.quantity(ItemId(3)) != 0
    }) || session.world().inventory(player_c).is_none_or(|inventory| {
        inventory.quantity(ItemId(3)) != 0
    }) {
        return Err("los inventarios no conservaron el aislamiento esperado".to_owned());
    }
    println!(
        "MUNDOS FUNCIONALES COMPLETADOS: recursos=3 unidades, comercio=compra/venta atomica, combate=enemigo={}, historia={:?}, saldos=[{},{},{}]",
        enemy,
        story_destination,
        session.world().player_currency(player_a).unwrap_or(0),
        session.world().player_currency(player_b).unwrap_or(0),
        session.world().player_currency(player_c).unwrap_or(0)
    );
    Ok(())
}

fn expect_transfer_rejection(
    session: &mut ProjectEchoSession,
    from: EntityId,
    to: EntityId,
    item: ItemId,
    amount: u32,
    expected: CommandStatus,
    label: &str,
) -> Result<(), String> {
    let before_from = session.world().inventory(from).cloned();
    let before_to = session.world().inventory(to).cloned();
    let before_active = session.world().entities().active_count();
    let before_players = session.world().players().count();
    let result = session
        .update([GameplayCommand::TransferInventoryItem {
            from,
            to,
            item,
            amount,
        }])
        .into_iter()
        .next()
        .ok_or_else(|| format!("{label}: no se devolvió resultado"))?;
    if result.status != expected {
        return Err(format!(
            "{label}: resultado inesperado: {:?}, esperado {:?}",
            result.status, expected
        ));
    }
    if session.world().inventory(from).cloned() != before_from
        || session.world().inventory(to).cloned() != before_to
        || session.world().entities().active_count() != before_active
        || session.world().players().count() != before_players
        || !session.world().events().is_empty()
    {
        return Err(format!("{label}: el rechazo dejó cambios parciales"));
    }
    Ok(())
}

fn run_inventory_transfer_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let sender = session.ids.first_player;
    let receiver = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador receptor: {error:?}"))?;
    let capacity_limited = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador con capacidad limitada: {error:?}"))?;
    session
        .world_mut()
        .initialize_inventory(receiver, 20)
        .map_err(|error| format!("inicializar inventario receptor: {error:?}"))?;
    session
        .world_mut()
        .initialize_inventory(capacity_limited, 1)
        .map_err(|error| format!("inicializar inventario limitado: {error:?}"))?;
    let item = ItemId(2);
    run_command(
        &mut session,
        "preparar inventario emisor",
        GameplayCommand::AddInventoryItem {
            player: sender,
            item,
            amount: 5,
        },
    )?;
    run_command(
        &mut session,
        "preparar inventario receptor",
        GameplayCommand::AddInventoryItem {
            player: receiver,
            item,
            amount: 2,
        },
    )?;
    run_command(
        &mut session,
        "preparar inventario limitado",
        GameplayCommand::AddInventoryItem {
            player: capacity_limited,
            item,
            amount: 1,
        },
    )?;

    let before_sender = session
        .world()
        .inventory(sender)
        .and_then(|inventory| Some(inventory.quantity(item)))
        .ok_or_else(|| "no se pudo leer la munición inicial del emisor".to_owned())?;
    let before_receiver = session
        .world()
        .inventory(receiver)
        .and_then(|inventory| Some(inventory.quantity(item)))
        .ok_or_else(|| "no se pudo leer la munición inicial del receptor".to_owned())?;
    let before_limited = session
        .world()
        .inventory(capacity_limited)
        .and_then(|inventory| Some(inventory.quantity(item)))
        .ok_or_else(|| "no se pudo leer el inventario limitado inicial".to_owned())?;

    run_command(
        &mut session,
        "transferir cantidad parcial",
        GameplayCommand::TransferInventoryItem {
            from: sender,
            to: receiver,
            item,
            amount: 3,
        },
    )?;
    if session.world().inventory(sender).map_or(0, |value| value.quantity(item)) != 2
        || session.world().inventory(receiver).map_or(0, |value| value.quantity(item)) != 5
        || session
            .world()
            .inventory(capacity_limited)
            .map_or(0, |value| value.quantity(item))
            != before_limited
    {
        return Err("la transferencia válida no actualizó exactamente los inventarios".to_owned());
    }
    let transfer_events = session.world().events().len();
    if transfer_events != 2 {
        return Err(format!(
            "la transferencia válida produjo {transfer_events} eventos en lugar de 2"
        ));
    }

    expect_transfer_rejection(
        &mut session,
        sender,
        receiver,
        item,
        0,
        CommandStatus::Invalid {
            reason: gameplay::CommandInvalidity::InvalidInventoryConfiguration,
        },
        "cantidad cero",
    )?;
    expect_transfer_rejection(
        &mut session,
        sender,
        receiver,
        item,
        99,
        CommandStatus::Rejected {
            reason: gameplay::CommandRejection::InventoryInsufficientQuantity,
        },
        "cantidad insuficiente",
    )?;
    expect_transfer_rejection(
        &mut session,
        sender,
        sender,
        item,
        1,
        CommandStatus::Rejected {
            reason: gameplay::CommandRejection::TransferToSelf,
        },
        "transferencia a sí mismo",
    )?;
    expect_transfer_rejection(
        &mut session,
        sender,
        receiver,
        ItemId(999),
        1,
        CommandStatus::Rejected {
            reason: gameplay::CommandRejection::ItemDefinitionNotFound,
        },
        "objeto inexistente",
    )?;
    expect_transfer_rejection(
        &mut session,
        EntityId::new(999_999),
        receiver,
        item,
        1,
        CommandStatus::Rejected {
            reason: gameplay::CommandRejection::PlayerNotFound,
        },
        "emisor inexistente",
    )?;
    expect_transfer_rejection(
        &mut session,
        sender,
        EntityId::new(999_999),
        item,
        1,
        CommandStatus::Rejected {
            reason: gameplay::CommandRejection::PlayerNotFound,
        },
        "receptor inexistente",
    )?;
    run_command(
        &mut session,
        "desactivar receptor para regresión",
        GameplayCommand::SetPlayerActive {
            player: receiver,
            active: false,
        },
    )?;
    expect_transfer_rejection(
        &mut session,
        sender,
        receiver,
        item,
        1,
        CommandStatus::Rejected {
            reason: gameplay::CommandRejection::PlayerInactive,
        },
        "receptor inactivo",
    )?;
    run_command(
        &mut session,
        "reactivar receptor después del rechazo",
        GameplayCommand::SetPlayerActive {
            player: receiver,
            active: true,
        },
    )?;
    expect_transfer_rejection(
        &mut session,
        sender,
        capacity_limited,
        item,
        1,
        CommandStatus::Rejected {
            reason: gameplay::CommandRejection::InventoryCapacityExceeded,
        },
        "capacidad del receptor",
    )?;

    if session.world().inventory(sender).map_or(0, |value| value.quantity(item))
        != before_sender - 3
        || session
            .world()
            .inventory(receiver)
            .map_or(0, |value| value.quantity(item))
            != before_receiver + 3
        || session
            .world()
            .inventory(capacity_limited)
            .map_or(0, |value| value.quantity(item))
            != before_limited
    {
        return Err("los rechazos posteriores alteraron cantidades válidas".to_owned());
    }
    if session.world().players().count() != 3 {
        return Err("la transferencia alteró el número de jugadores".to_owned());
    }
    println!(
        "TRANSFERENCIAS COMPLETADAS: emisor={}, receptor={}, limitado={}, emisor_restante={}, receptor_final={}, limitado_final={}",
        sender,
        receiver,
        capacity_limited,
        session.world().inventory(sender).map_or(0, |value| value.quantity(item)),
        session.world().inventory(receiver).map_or(0, |value| value.quantity(item)),
        session
            .world()
            .inventory(capacity_limited)
            .map_or(0, |value| value.quantity(item))
    );
    Ok(())
}

fn run_reward_distribution_scenario() -> Result<(), String> {
    let item = gameplay::types::ItemId(3);
    let weapon = WeaponConfiguration::with_ammunition(
        AttackType::Kinetic,
        100,
        20,
        Duration(1_000),
        WeaponAmmunition {
            item: gameplay::types::ItemId(2),
            amount_per_attack: 1,
        },
    );

    let mut shared_session = ProjectEchoSession::new();
    let shared_player = shared_session.ids.first_player;
    let shared_player_b = shared_session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador compartido B: {error:?}"))?;
    let shared_player_c = shared_session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador compartido C: {error:?}"))?;
    for player in [shared_player_b, shared_player_c] {
        shared_session
            .world_mut()
            .initialize_inventory(player, 20)
            .map_err(|error| format!("inicializar inventario compartido: {error:?}"))?;
    }
    run_command(
        &mut shared_session,
        "dar munición para recompensa compartida",
        GameplayCommand::AddInventoryItem {
            player: shared_player,
            item: gameplay::types::ItemId(2),
            amount: 1,
        },
    )?;
    run_command(
        &mut shared_session,
        "equipar arma para recompensa compartida",
        GameplayCommand::EquipPlayerWeapon {
            player: shared_player,
            weapon,
        },
    )?;
    run_command(
        &mut shared_session,
        "crear enemigo de recompensa compartida",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let shared_enemy = shared_session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "no se creó el enemigo de recompensa compartida".to_owned())?;
    run_command(
        &mut shared_session,
        "derrotar enemigo de recompensa compartida",
        GameplayCommand::AttackEnemy {
            player: shared_player,
            enemy: shared_enemy,
        },
    )?;
    let shared_rewards: Vec<_> = shared_session
        .world()
        .collectibles()
        .filter(|collectible| collectible.item == item)
        .copied()
        .collect();
    if shared_rewards.len() != 1 || shared_rewards[0].owner.is_some() {
        return Err("la recompensa compartida no generó un único objeto accesible".to_owned());
    }
    let shared_collectible = shared_rewards[0];
    let shared_before_b = shared_session
        .world()
        .inventory(shared_player_b)
        .map_or(0, |inventory| inventory.quantity(item));
    let shared_before_c = shared_session
        .world()
        .inventory(shared_player_c)
        .map_or(0, |inventory| inventory.quantity(item));
    run_command(
        &mut shared_session,
        "recoger recompensa compartida",
        GameplayCommand::CollectItem {
            player: shared_player,
            collectible: shared_collectible.entity,
            amount: 1,
        },
    )?;
    if shared_session
        .world()
        .inventory(shared_player)
        .map_or(0, |inventory| inventory.quantity(item))
        != 1
        || shared_session
            .world()
            .inventory(shared_player_b)
            .map_or(0, |inventory| inventory.quantity(item))
            != shared_before_b
        || shared_session
            .world()
            .inventory(shared_player_c)
            .map_or(0, |inventory| inventory.quantity(item))
            != shared_before_c
        || shared_session
            .world()
            .collectible(shared_collectible.entity)
            .is_none_or(|collectible| collectible.active || collectible.amount != 0)
    {
        return Err("la recompensa compartida no se recogió exactamente una vez".to_owned());
    }
    let shared_retry = shared_session.update([GameplayCommand::CollectItem {
        player: shared_player,
        collectible: shared_collectible.entity,
        amount: 1,
    }]);
    if !matches!(
        shared_retry.first().map(|result| result.status),
        Some(gameplay::CommandStatus::Rejected {
            reason: gameplay::CommandRejection::CollectibleUnavailable,
        })
    ) {
        return Err("la recompensa compartida pudo reclamarse dos veces".to_owned());
    }

    let mut personal_session = ProjectEchoSession::new();
    personal_session
        .world_mut()
        .configure_enemy_reward_distribution(
            EnemyArchetype::Shooter,
            RewardDistributionPolicy::Personal,
        )
        .map_err(|error| format!("configurar recompensa personal: {error:?}"))?;
    let personal_a = personal_session.ids.first_player;
    let personal_b = personal_session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador personal B: {error:?}"))?;
    let personal_c = personal_session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador personal C: {error:?}"))?;
    for player in [personal_b, personal_c] {
        personal_session
            .world_mut()
            .initialize_inventory(player, 20)
            .map_err(|error| format!("inicializar inventario personal: {error:?}"))?;
    }
    run_command(
        &mut personal_session,
        "dar munición para recompensa personal",
        GameplayCommand::AddInventoryItem {
            player: personal_a,
            item: gameplay::types::ItemId(2),
            amount: 1,
        },
    )?;
    run_command(
        &mut personal_session,
        "equipar arma para recompensa personal",
        GameplayCommand::EquipPlayerWeapon {
            player: personal_a,
            weapon,
        },
    )?;
    run_command(
        &mut personal_session,
        "crear enemigo de recompensa personal",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let personal_enemy = personal_session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "no se creó el enemigo de recompensa personal".to_owned())?;
    run_command(
        &mut personal_session,
        "derrotar enemigo de recompensa personal",
        GameplayCommand::AttackEnemy {
            player: personal_a,
            enemy: personal_enemy,
        },
    )?;
    let personal_rewards: Vec<_> = personal_session
        .world()
        .collectibles()
        .filter(|collectible| collectible.item == item)
        .copied()
        .collect();
    if personal_rewards.len() != 3
        || ![personal_a, personal_b, personal_c]
            .iter()
            .all(|player| personal_rewards.iter().any(|reward| reward.owner == Some(*player)))
    {
        return Err("la recompensa personal no generó una concesión por jugador elegible".to_owned());
    }
    let reward_a = personal_rewards
        .iter()
        .find(|reward| reward.owner == Some(personal_a))
        .copied()
        .ok_or_else(|| "falta la recompensa personal de A".to_owned())?;
    let before_personal_a = personal_session
        .world()
        .inventory(personal_a)
        .cloned()
        .ok_or_else(|| "falta inventario personal de A".to_owned())?;
    let before_personal_b = personal_session
        .world()
        .inventory(personal_b)
        .cloned()
        .ok_or_else(|| "falta inventario personal de B".to_owned())?;
    let rejected_personal = personal_session.update([GameplayCommand::CollectItem {
        player: personal_b,
        collectible: reward_a.entity,
        amount: 1,
    }]);
    if !matches!(
        rejected_personal.first().map(|result| result.status),
        Some(gameplay::CommandStatus::Rejected {
            reason: gameplay::CommandRejection::CollectibleNotOwned,
        })
    ) || personal_session.world().inventory(personal_a) != Some(&before_personal_a)
        || personal_session.world().inventory(personal_b) != Some(&before_personal_b)
        || personal_session.world().collectible(reward_a.entity) != Some(&reward_a)
        || !personal_session.world().events().is_empty()
    {
        return Err("la recogida ajena de una recompensa personal no fue atómica".to_owned());
    }
    for player in [personal_a, personal_b, personal_c] {
        let reward = personal_rewards
            .iter()
            .find(|reward| reward.owner == Some(player))
            .copied()
            .ok_or_else(|| "falta una recompensa personal durante la recogida".to_owned())?;
        run_command(
            &mut personal_session,
            "recoger recompensa personal propia",
            GameplayCommand::CollectItem {
                player,
                collectible: reward.entity,
                amount: 1,
            },
        )?;
    }
    let retry_personal = personal_session.update([GameplayCommand::CollectItem {
        player: personal_a,
        collectible: reward_a.entity,
        amount: 1,
    }]);
    if !matches!(
        retry_personal.first().map(|result| result.status),
        Some(gameplay::CommandStatus::Rejected {
            reason: gameplay::CommandRejection::CollectibleUnavailable,
        })
    ) || [personal_a, personal_b, personal_c].iter().any(|player| {
        personal_session
            .world()
            .inventory(*player)
            .map_or(0, |inventory| inventory.quantity(item))
            != 1
    })
    {
        return Err("la recompensa personal no conservó cantidades o idempotencia".to_owned());
    }
    let repeated_attack = personal_session.update([GameplayCommand::AttackEnemy {
        player: personal_a,
        enemy: personal_enemy,
    }]);
    if !matches!(
        repeated_attack.first().map(|result| result.status),
        Some(gameplay::CommandStatus::Rejected {
            reason: gameplay::CommandRejection::CombatantInactive,
        })
    ) || personal_session.world().collectibles().count() != 3
        || !personal_session.world().events().is_empty()
    {
        return Err("reprocesar una derrota generó recompensas duplicadas".to_owned());
    }
    println!(
        "RECOMPENSAS COMPLETADAS: compartida=1, personales=3, propietarios={},{},{}",
        personal_a, personal_b, personal_c
    );
    Ok(())
}

fn run_autonomous_enemy_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player_a = session.ids.first_player;
    let player_b = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador de IA B: {error:?}"))?;
    session
        .world_mut()
        .initialize_inventory(player_b, 20)
        .map_err(|error| format!("inicializar inventario de IA B: {error:?}"))?;
    run_command(
        &mut session,
        "crear enemigo tirador autónomo",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let shooter = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "no se creó el tirador autónomo".to_owned())?;

    run_command(
        &mut session,
        "avanzar para primer ataque autónomo",
        GameplayCommand::AdvanceTime {
            delta: Duration(800),
        },
    )?;
    let health_after_first = session
        .world()
        .player_health(player_a)
        .map(|health| health.0)
        .ok_or_else(|| "no se pudo leer la salud tras el primer ataque".to_owned())?;
    if health_after_first != 92
        || !session.world().events().iter().any(|event| {
            matches!(
                event,
                GameplayEvent::AttackExecuted { attacker, target, .. }
                    if *attacker == shooter && *target == player_a
            )
        })
    {
        return Err("el enemigo autónomo no adquirió al jugador A de forma determinista".to_owned());
    }
    run_command(
        &mut session,
        "avanzar dentro del cooldown autónomo",
        GameplayCommand::AdvanceTime {
            delta: Duration(100),
        },
    )?;
    if session
        .world()
        .player_health(player_a)
        .map(|health| health.0)
        != Some(92)
        || session.world().events().iter().any(|event| {
            matches!(
                event,
                GameplayEvent::AttackExecuted { attacker, .. } if *attacker == shooter
            )
        })
    {
        return Err("el enemigo autónomo ignoró su cooldown".to_owned());
    }
    run_command(
        &mut session,
        "avanzar para segundo ataque autónomo",
        GameplayCommand::AdvanceTime {
            delta: Duration(700),
        },
    )?;
    if session
        .world()
        .player_health(player_a)
        .map(|health| health.0)
        != Some(84)
    {
        return Err("el segundo ataque autónomo no respetó el tiempo lógico".to_owned());
    }
    run_command(
        &mut session,
        "invalidar objetivo autónomo A",
        GameplayCommand::SetPlayerActive {
            player: player_a,
            active: false,
        },
    )?;
    run_command(
        &mut session,
        "seleccionar nuevo objetivo autónomo",
        GameplayCommand::AdvanceTime {
            delta: Duration(800),
        },
    )?;
    if session
        .world()
        .player_health(player_b)
        .map(|health| health.0)
        != Some(92)
        || session
            .world()
            .player_health(player_a)
            .map(|health| health.0)
            != Some(84)
        || !session.world().events().iter().any(|event| {
            matches!(
                event,
                GameplayEvent::AttackExecuted { attacker, target, .. }
                    if *attacker == shooter && *target == player_b
            )
        })
    {
        return Err("la IA no re-seleccionó un objetivo válido tras la desactivación".to_owned());
    }

    run_command(
        &mut session,
        "dar munición para derrotar al tirador",
        GameplayCommand::AddInventoryItem {
            player: player_b,
            item: gameplay::types::ItemId(2),
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "equipar arma contra el tirador",
        GameplayCommand::EquipPlayerWeapon {
            player: player_b,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                100,
                20,
                Duration(1_000),
                WeaponAmmunition {
                    item: gameplay::types::ItemId(2),
                    amount_per_attack: 1,
                },
            ),
        },
    )?;
    run_command(
        &mut session,
        "derrotar tirador después del encuentro autónomo",
        GameplayCommand::AttackEnemy {
            player: player_b,
            enemy: shooter,
        },
    )?;
    let reward_count = session.world().collectibles().count();
    if session
        .world()
        .combatant(shooter)
        .is_none_or(|combatant| combatant.active || !combatant.is_defeated())
        || reward_count != 1
    {
        return Err("la derrota posterior a la IA no produjo el estado o recompensa esperados".to_owned());
    }
    run_command(
        &mut session,
        "avanzar con enemigo derrotado",
        GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        },
    )?;
    if session.world().collectibles().count() != reward_count
        || session.world().events().iter().any(|event| {
            matches!(
                event,
                GameplayEvent::AttackExecuted { attacker, .. } if *attacker == shooter
            )
        })
    {
        return Err("el enemigo derrotado continuó ejecutando acciones autónomas".to_owned());
    }

    run_command(
        &mut session,
        "crear cargador autónomo",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(
                EnemyArchetype::Charger,
                Vec3 {
                    x: 10.0,
                    y: 0.0,
                    z: 0.0,
                },
            ),
        },
    )?;
    let charger = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, archetype, .. }
                if *archetype == EnemyArchetype::Charger => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "no se creó el cargador autónomo".to_owned())?;
    run_command(
        &mut session,
        "mover cargador autónomo",
        GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        },
    )?;
    let charger_position = session
        .world()
        .combatant(charger)
        .map(|combatant| combatant.position)
        .ok_or_else(|| "desapareció el cargador autónomo".to_owned())?;
    if charger_position.x >= 10.0
        || !session.world().events().iter().any(|event| {
            matches!(event, GameplayEvent::EnemyMoved { enemy, .. } if *enemy == charger)
        })
    {
        return Err("el cargador no ejecutó movimiento táctico observable".to_owned());
    }
    println!(
        "IA AUTÓNOMA COMPLETADA: tirador={}, objetivo_inicial={}, objetivo_reseleccionado={}, cargador={}, posicion_cargador={:.1}",
        shooter, player_a, player_b, charger, charger_position.x
    );
    Ok(())
}

fn run_spatial_encounter_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let ship = session.ids.first_ship;
    let position = Vec3::ZERO;
    let hazard = gameplay::HazardId::new(901);
    run_command(
        &mut session,
        "registrar peligro de encuentro",
        GameplayCommand::RegisterEnvironmentalHazard {
            hazard: EnvironmentalHazard::new(
                hazard,
                gameplay::EnvironmentalHazardType::SpaceStorm,
                position,
                2.0,
                7,
                Duration(10_000),
                true,
            ),
        },
    )?;
    let hazard_encounter = EncounterId::new(901);
    run_command(
        &mut session,
        "registrar encuentro de peligro",
        GameplayCommand::RegisterSpatialEncounter {
            encounter: SpatialEncounter::new(
                hazard_encounter,
                SpatialEncounterKind::Hazard { hazard },
                position,
                2.0,
            ),
        },
    )?;
    run_command(
        &mut session,
        "activar encuentro de peligro",
        GameplayCommand::ActivateSpatialEncounter {
            encounter: hazard_encounter,
        },
    )?;
    let integrity_before = session
        .world()
        .ship_integrity(session.ids.first_ship)
        .ok_or_else(|| "falta integridad para encuentro".to_owned())?
        .current();
    run_command(
        &mut session,
        "resolver encuentro de peligro",
        GameplayCommand::ResolveSpatialEncounter {
            player,
            encounter: hazard_encounter,
        },
    )?;
    let integrity_after = session
        .world()
        .ship_integrity(session.ids.first_ship)
        .ok_or_else(|| "falta integridad después del encuentro".to_owned())?
        .current();
    if integrity_after >= integrity_before
        || session.world().spatial_encounter(hazard_encounter).map(|value| value.state)
            != Some(SpatialEncounterState::Resolved)
    {
        return Err("el peligro no produjo una resolución ni daño real".to_owned());
    }
    let repeated = session
        .update([GameplayCommand::ResolveSpatialEncounter {
            player,
            encounter: hazard_encounter,
        }])
        .into_iter()
        .next()
        .ok_or_else(|| "repetir peligro no devolvió resultado".to_owned())?;
    if repeated.status
        != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::SpatialEncounterAlreadyResolved,
        })
    {
        return Err(format!("resolución repetida inesperada: {:?}", repeated.status));
    }
    if session
        .world()
        .ship_integrity(session.ids.first_ship)
        .is_none_or(|value| value.current() != integrity_after)
    {
        return Err("el peligro repetido modificó la integridad".to_owned());
    }

    let item = ShipRepairConfiguration::default().material;
    let resource_encounter = EncounterId::new(902);
    run_command(
        &mut session,
        "registrar encuentro de recursos",
        GameplayCommand::RegisterSpatialEncounter {
            encounter: SpatialEncounter::new(
                resource_encounter,
                SpatialEncounterKind::Resource { item, amount: 1 },
                position,
                2.0,
            ),
        },
    )?;
    run_command(
        &mut session,
        "activar encuentro de recursos",
        GameplayCommand::ActivateSpatialEncounter {
            encounter: resource_encounter,
        },
    )?;
    let collectibles_before = session.world().collectibles().count();
    run_command(
        &mut session,
        "resolver encuentro de recursos",
        GameplayCommand::ResolveSpatialEncounter {
            player,
            encounter: resource_encounter,
        },
    )?;
    let collectible = session
        .world()
        .collectibles()
        .find(|value| value.active)
        .map(|value| value.entity)
        .ok_or_else(|| "el hallazgo no creó un objeto recuperable".to_owned())?;
    if session.world().collectibles().count() != collectibles_before + 1 {
        return Err("el hallazgo creó una cantidad inesperada de objetos".to_owned());
    }
    run_command(
        &mut session,
        "recoger hallazgo de recursos",
        GameplayCommand::CollectItem {
            player,
            collectible,
            amount: 1,
        },
    )?;
    if session.world().collectible(collectible).is_none_or(|value| value.active) {
        return Err("el hallazgo no quedó agotado tras recogerlo".to_owned());
    }
    let repeated_resource = session
        .update([GameplayCommand::ResolveSpatialEncounter {
            player,
            encounter: resource_encounter,
        }])
        .into_iter()
        .next()
        .ok_or_else(|| "repetir hallazgo no devolvió resultado".to_owned())?;
    if repeated_resource.status
        != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::SpatialEncounterAlreadyResolved,
        })
    {
        return Err(format!("hallazgo repetido inesperado: {:?}", repeated_resource.status));
    }

    let destination = DestinationId::new(903);
    session
        .register_destination(Destination {
            id: destination,
            destination_type: DestinationType::SpacePoint,
            position,
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Unknown,
        })
        .map_err(|error| format!("registrar destino de señal: {error:?}"))?;
    run_command(
        &mut session,
        "registrar conocimiento de señal",
        GameplayCommand::RegisterKnowledge {
            target: gameplay::KnowledgeTarget::Destination(destination),
        },
    )?;
    let signal_encounter = EncounterId::new(903);
    run_command(
        &mut session,
        "registrar encuentro de señal",
        GameplayCommand::RegisterSpatialEncounter {
            encounter: SpatialEncounter::new(
                signal_encounter,
                SpatialEncounterKind::Signal { destination },
                position,
                2.0,
            ),
        },
    )?;
    run_command(
        &mut session,
        "activar encuentro de señal",
        GameplayCommand::ActivateSpatialEncounter {
            encounter: signal_encounter,
        },
    )?;
    run_command(
        &mut session,
        "detectar señal del encuentro",
        GameplayCommand::ScanSignals {
            player,
            ship,
        },
    )?;
    run_command(
        &mut session,
        "identificar señal",
        GameplayCommand::IdentifyKnowledge {
            target: gameplay::KnowledgeTarget::Destination(destination),
        },
    )?;
    if session.world().spatial_encounter(signal_encounter).map(|value| value.state)
        != Some(SpatialEncounterState::Resolved)
    {
        return Err("la identificación no resolvió el encuentro de señal".to_owned());
    }
    println!(
        "ENCUENTROS ESPACIALES COMPLETADOS: integridad={}→{}, hallazgo_recogido=true, señal=Resuelta",
        integrity_before, integrity_after
    );
    Ok(())
}

fn run_autopilot_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let ship = session.ids.first_ship;
    let station = session
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estación de autopilot: {error:?}"))?;
    run_command(
        &mut session,
        "ocupar estación de autopilot",
        GameplayCommand::Interact {
            player,
            target: station,
        },
    )?;
    let home = DestinationId::new(1001);
    let first = DestinationId::new(1002);
    let second = DestinationId::new(1003);
    let undiscovered = DestinationId::new(1004);
    for destination in [
        Destination {
            id: home,
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        },
        Destination {
            id: first,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
            travel_duration: Duration(10),
            fuel_cost: 1,
            discovery: DiscoveryState::Known,
        },
        Destination {
            id: second,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 20.0, y: 0.0, z: 0.0 },
            travel_duration: Duration(10),
            fuel_cost: 1,
            discovery: DiscoveryState::Known,
        },
        Destination {
            id: undiscovered,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 30.0, y: 0.0, z: 0.0 },
            travel_duration: Duration(10),
            fuel_cost: 1,
            discovery: DiscoveryState::Known,
        },
    ] {
        session
            .register_destination(destination)
            .map_err(|error| format!("registrar destino de autopilot: {error:?}"))?;
    }
    run_command(
        &mut session,
        "seleccionar primer planeta",
        GameplayCommand::SelectDestination {
            player,
            pilot_station: station,
            destination: first,
        },
    )?;
    run_command(
        &mut session,
        "iniciar descubrimiento del primer planeta",
        GameplayCommand::StartTravel {
            player,
            pilot_station: station,
        },
    )?;
    run_command(
        &mut session,
        "llegar al primer planeta",
        GameplayCommand::AdvanceTime {
            delta: Duration(10),
        },
    )?;
    if session.world().destination(first).map(|value| value.discovery)
        != Some(DiscoveryState::Discovered)
    {
        return Err("el primer planeta no se descubrió físicamente".to_owned());
    }
    run_command(
        &mut session,
        "seleccionar segundo planeta",
        GameplayCommand::SelectDestination {
            player,
            pilot_station: station,
            destination: second,
        },
    )?;
    run_command(
        &mut session,
        "iniciar descubrimiento del segundo planeta",
        GameplayCommand::StartTravel {
            player,
            pilot_station: station,
        },
    )?;
    run_command(
        &mut session,
        "llegar al segundo planeta",
        GameplayCommand::AdvanceTime {
            delta: Duration(10),
        },
    )?;
    if session.world().destination(second).map(|value| value.discovery)
        != Some(DiscoveryState::Discovered)
    {
        return Err("el segundo planeta no se descubrió físicamente".to_owned());
    }
    let route = |from, to, distance, duration, fuel_cost| NavigationRoute {
        from,
        to,
        distance,
        duration: Duration(duration),
        fuel_cost,
        state: NavigationRouteState::Available,
    };
    for value in [
        route(home, first, 10.0, 4, 2),
        route(first, second, 10.0, 4, 2),
        route(second, first, 10.0, 4, 2),
        route(first, home, 10.0, 4, 2),
        route(second, home, 20.0, 12, 5),
    ] {
        run_command(
            &mut session,
            "registrar ruta descubierta",
            GameplayCommand::RegisterNavigationRoute { route: value },
        )?;
    }
    let duplicate = session
        .update([GameplayCommand::RegisterNavigationRoute {
            route: route(second, first, 10.0, 4, 2),
        }])
        .into_iter()
        .next()
        .ok_or_else(|| "ruta duplicada sin resultado".to_owned())?;
    if duplicate.status
        != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::NavigationRouteAlreadyRegistered,
        })
    {
        return Err(format!("ruta duplicada no rechazada: {:?}", duplicate.status));
    }
    let unknown_route = session
        .update([GameplayCommand::RegisterNavigationRoute {
            route: route(second, undiscovered, 10.0, 4, 2),
        }])
        .into_iter()
        .next()
        .ok_or_else(|| "ruta no descubierta sin resultado".to_owned())?;
    if unknown_route.status
        != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::NavigationRouteNotDiscovered,
        })
    {
        return Err(format!("ruta no descubierta no rechazada: {:?}", unknown_route.status));
    }
    let inaccessible = session
        .update([GameplayCommand::PlanAutopilot {
            player,
            pilot_station: station,
            origin: second,
            destination: undiscovered,
        }])
        .into_iter()
        .next()
        .ok_or_else(|| "plan inaccesible sin resultado".to_owned())?;
    if inaccessible.status
        != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::AutopilotNoRoute,
        })
    {
        return Err(format!("destino inaccesible no rechazado: {:?}", inaccessible.status));
    }
    run_command(
        &mut session,
        "planificar autopilot por rutas conocidas",
        GameplayCommand::PlanAutopilot {
            player,
            pilot_station: station,
            origin: second,
            destination: home,
        },
    )?;
    match session.world().autopilot_state() {
        AutopilotState::Planned { eta, route, .. } if *eta == Duration(8) && route.as_slice() == [second, first, home] => {}
        state => return Err(format!("plan determinista inesperado: {state:?}")),
    }
    let fuel_before = session
        .world()
        .ship_fuel(ship)
        .ok_or_else(|| "faltan recursos del autopilot".to_owned())?
        .current();
    run_command(
        &mut session,
        "iniciar autopilot",
        GameplayCommand::StartAutopilot {
            player,
            pilot_station: station,
        },
    )?;
    let fuel_after_start = session
        .world()
        .ship_fuel(ship)
        .ok_or_else(|| "faltan recursos después de iniciar autopilot".to_owned())?
        .current();
    if fuel_before - fuel_after_start != 2 {
        return Err("el primer tramo no consumió el combustible configurado".to_owned());
    }
    run_command(
        &mut session,
        "avanzar primer tramo del autopilot",
        GameplayCommand::AdvanceTime {
            delta: Duration(4),
        },
    )?;
    if !matches!(session.world().autopilot_state(), AutopilotState::InTransit { .. })
        || session.world().ship_position() != Some(Vec3 { x: 10.0, y: 0.0, z: 0.0 })
    {
        return Err("el autopilot no encadenó el segundo tramo".to_owned());
    }
    run_command(
        &mut session,
        "avanzar tramo final del autopilot",
        GameplayCommand::AdvanceTime {
            delta: Duration(4),
        },
    )?;
    if !matches!(
        session.world().autopilot_state(),
        AutopilotState::Completed { destination } if *destination == home
    ) || session.world().ship_position() != Some(Vec3::ZERO)
    {
        return Err("el autopilot no completó en el destino final".to_owned());
    }
    let fuel_after = session
        .world()
        .ship_fuel(ship)
        .ok_or_else(|| "faltan recursos al finalizar autopilot".to_owned())?
        .current();
    if fuel_before - fuel_after != 4 {
        return Err("el autopilot consumió combustible incorrectamente".to_owned());
    }
    run_command(
        &mut session,
        "retirar combustible para validar rechazo",
        GameplayCommand::RemoveShipFuel { ship, amount: 53 },
    )?;
    run_command(
        &mut session,
        "planificar ruta con combustible insuficiente",
        GameplayCommand::PlanAutopilot {
            player,
            pilot_station: station,
            origin: home,
            destination: first,
        },
    )?;
    let insufficient = session
        .update([GameplayCommand::StartAutopilot {
            player,
            pilot_station: station,
        }])
        .into_iter()
        .next()
        .ok_or_else(|| "rechazo por combustible sin resultado".to_owned())?;
    if insufficient.status
        != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::InsufficientFuel,
        })
        || session.world().ship_fuel(ship).map(|fuel| fuel.current()) != Some(1)
    {
        return Err(format!("combustible insuficiente no fue atómico: {:?}", insufficient.status));
    }
    run_command(
        &mut session,
        "cancelar plan sin combustible",
        GameplayCommand::CancelAutopilot {
            player,
            pilot_station: station,
        },
    )?;
    let hostile_encounter = EncounterId::new(1005);
    run_command(
        &mut session,
        "registrar contacto hostil",
        GameplayCommand::RegisterSpatialEncounter {
            encounter: SpatialEncounter::new(
                hostile_encounter,
                SpatialEncounterKind::Hostile {
                    configuration: EnemyConfiguration::for_archetype(
                        EnemyArchetype::Shooter,
                        Vec3::ZERO,
                    ),
                },
                Vec3::ZERO,
                2.0,
            ),
        },
    )?;
    run_command(
        &mut session,
        "activar contacto hostil",
        GameplayCommand::ActivateSpatialEncounter {
            encounter: hostile_encounter,
        },
    )?;
    let hostile = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, archetype, .. }
                if *archetype == EnemyArchetype::Shooter => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "el contacto hostil no creó enemigo".to_owned())?;
    run_command(
        &mut session,
        "equipar arma del contacto hostil",
        GameplayCommand::EquipPlayerWeapon {
            player,
            weapon: WeaponConfiguration::new(AttackType::Energy, 60, 20, Duration(1)),
        },
    )?;
    run_command(
        &mut session,
        "derrotar enemigo del contacto hostil",
        GameplayCommand::AttackEnemy {
            player,
            enemy: hostile,
        },
    )?;
    if !session
        .world()
        .combatant(hostile)
        .is_some_and(|combatant| combatant.is_defeated())
        || session.world().spatial_encounter(hostile_encounter).map(|value| value.state)
            != Some(SpatialEncounterState::Resolved)
    {
        return Err("el contacto hostil no se resolvió con la derrota real".to_owned());
    }
    let isolated = ProjectEchoSession::new();
    if isolated.world().navigation_routes().next().is_some() {
        return Err("las rutas se filtraron entre sesiones".to_owned());
    }
    println!(
        "AUTOPILOT COMPLETADO: ruta={:?}, eta=8, combustible={}→{}, destino_final={:?}, hostil_resuelto=true",
        [second, first, home], fuel_before, fuel_after, home
    );
    Ok(())
}

fn run_integrated_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let ship = session.ids.first_ship;
    let pilot_station = session
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estación de pilotaje integrada: {error:?}"))?;
    let travel_destination = DestinationId::new(101);
    let signal_destination = DestinationId::new(102);
    let travel_position = Vec3 {
        x: 10.0,
        y: 0.0,
        z: 0.0,
    };
    let signal_position = Vec3 {
        x: 25.0,
        y: 0.0,
        z: 0.0,
    };
    session
        .register_destination(Destination {
            id: travel_destination,
            destination_type: DestinationType::Planet,
            position: travel_position,
            travel_duration: Duration(100),
            fuel_cost: 5,
            discovery: DiscoveryState::Known,
        })
        .map_err(|error| format!("registrar destino de viaje: {error:?}"))?;
    session
        .register_destination(Destination {
            id: signal_destination,
            destination_type: DestinationType::SpacePoint,
            position: signal_position,
            travel_duration: Duration(200),
            fuel_cost: 8,
            discovery: DiscoveryState::Unknown,
        })
        .map_err(|error| format!("registrar señal espacial: {error:?}"))?;

    run_command(
        &mut session,
        "ocupar pilotaje integrado",
        GameplayCommand::Interact {
            player,
            target: pilot_station,
        },
    )?;
    run_command(
        &mut session,
        "escanear señal inicial",
        GameplayCommand::ScanSignals { player, ship },
    )?;
    if session
        .world()
        .navigation()
        .signal_knowledge(signal_destination)
        != SignalKnowledge::Detected
    {
        return Err("la señal desconocida no quedó detectada".to_owned());
    }

    let fuel_before = session
        .world()
        .ship_fuel(ship)
        .ok_or_else(|| "no hay combustible inicial".to_owned())?
        .current();
    run_command(
        &mut session,
        "seleccionar destino conocido",
        GameplayCommand::SelectDestination {
            player,
            pilot_station,
            destination: travel_destination,
        },
    )?;
    run_command(
        &mut session,
        "iniciar viaje integrado",
        GameplayCommand::StartTravel {
            player,
            pilot_station,
        },
    )?;
    let fuel_after_start = session
        .world()
        .ship_fuel(ship)
        .ok_or_else(|| "no hay combustible después del inicio de viaje".to_owned())?
        .current();
    if fuel_after_start != fuel_before.saturating_sub(5)
        || !matches!(
            session.world().travel_state(),
            TravelState::InProgress { .. }
        )
    {
        return Err("el viaje o el consumo de combustible no fueron coherentes".to_owned());
    }
    run_command(
        &mut session,
        "completar llegada planetaria",
        GameplayCommand::AdvanceTime {
            delta: Duration(100),
        },
    )?;
    if !matches!(
        session.world().travel_state(),
        TravelState::Completed {
            destination: destination_id
        } if destination_id == travel_destination
    ) || session.world().ship_position() != Some(travel_position)
        || session
            .world()
            .destination(travel_destination)
            .map(|destination| destination.discovery)
            != Some(DiscoveryState::Discovered)
    {
        return Err("la llegada no actualizó posición, viaje y descubrimiento".to_owned());
    }

    run_command(
        &mut session,
        "esperar recarga del escáner",
        GameplayCommand::AdvanceTime {
            delta: Duration(900),
        },
    )?;
    run_command(
        &mut session,
        "identificar señal tras la llegada",
        GameplayCommand::ScanSignals { player, ship },
    )?;
    if session
        .world()
        .navigation()
        .signal_knowledge(signal_destination)
        != SignalKnowledge::Identified
    {
        return Err("la señal detectada no quedó identificada".to_owned());
    }

    let standard_ammunition = gameplay::types::ItemId(2);
    run_command(
        &mut session,
        "añadir munición estándar",
        GameplayCommand::AddInventoryItem {
            player,
            item: standard_ammunition,
            amount: 1,
        },
    )?;
    let weapon = WeaponConfiguration::with_ammunition(
        AttackType::Kinetic,
        60,
        20,
        Duration(1_000),
        WeaponAmmunition {
            item: standard_ammunition,
            amount_per_attack: 1,
        },
    );
    run_command(
        &mut session,
        "equipar arma estándar",
        GameplayCommand::EquipPlayerWeapon { player, weapon },
    )?;
    run_command(
        &mut session,
        "crear enemigo tirador",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let enemy = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "la creación del enemigo no publicó su evento".to_owned())?;
    let health_before = session
        .world()
        .combatant(enemy)
        .ok_or_else(|| "el enemigo creado no tiene combatiente".to_owned())?
        .current_health;
    if health_before != 60 {
        return Err(format!(
            "salud inicial inesperada del enemigo: {health_before}"
        ));
    }
    run_command(
        &mut session,
        "atacar enemigo tirador",
        GameplayCommand::AttackEnemy { player, enemy },
    )?;
    let ammunition_after = session
        .world()
        .inventory(player)
        .ok_or_else(|| "el jugador no tiene inventario".to_owned())?
        .quantity(standard_ammunition);
    let enemy_after = session
        .world()
        .combatant(enemy)
        .ok_or_else(|| "desapareció el combatiente enemigo".to_owned())?;
    if ammunition_after != 0 || enemy_after.current_health != 0 || enemy_after.active {
        return Err("el ataque no produjo el daño o consumo esperado".to_owned());
    }
    let reward_collectible = session
        .world()
        .collectibles()
        .find(|collectible| collectible.item == gameplay::types::ItemId(3))
        .copied()
        .ok_or_else(|| "la derrota no generó la recompensa esperada".to_owned())?;
    if reward_collectible.amount != 1 || !reward_collectible.active {
        return Err("la recompensa generada tiene un estado inválido".to_owned());
    }
    run_command(
        &mut session,
        "recoger recompensa de combate",
        GameplayCommand::CollectItem {
            player,
            collectible: reward_collectible.entity,
            amount: 1,
        },
    )?;
    if session
        .world()
        .inventory(player)
        .is_none_or(|inventory| inventory.quantity(gameplay::types::ItemId(3)) != 1)
        || session
            .world()
            .collectible(reward_collectible.entity)
            .is_none_or(|collectible| collectible.active || collectible.amount != 0)
    {
        return Err("la recolección no actualizó inventario y objeto".to_owned());
    }
    println!(
        "ESCENARIO INTEGRADO COMPLETADO: destino={:?}, señal={:?}, enemigo={}, munición_restante={}, recompensa_recogida=1",
        travel_destination,
        signal_destination,
        enemy,
        ammunition_after
    );
    Ok(())
}

fn run_anomaly_mission_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let ship = session.ids.first_ship;
    let pilot_station = session
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estacion de la anomalia: {error:?}"))?;
    let travel_position = Vec3 {
        x: 10.0,
        y: 0.0,
        z: 0.0,
    };
    session
        .register_destination(Destination {
            id: ecos_anomalia::PLANET_DESTINATION,
            destination_type: DestinationType::Planet,
            position: travel_position,
            travel_duration: Duration(100),
            fuel_cost: 5,
            discovery: DiscoveryState::Known,
        })
        .map_err(|error| format!("registrar planeta de la anomalia: {error:?}"))?;
    session
        .register_destination(Destination {
            id: ecos_anomalia::SIGNAL_DESTINATION,
            destination_type: DestinationType::SpacePoint,
            position: Vec3 {
                x: 25.0,
                y: 0.0,
                z: 0.0,
            },
            travel_duration: Duration(200),
            fuel_cost: 8,
            discovery: DiscoveryState::Unknown,
        })
        .map_err(|error| format!("registrar senal de la anomalia: {error:?}"))?;

    let initial = session
        .mission_progress(ecos_anomalia::ECOS_DE_LA_ANOMALIA)
        .ok_or_else(|| "no existe Ecos de la anomalia".to_owned())?;
    if initial.state != MissionState::Locked {
        return Err(format!(
            "Ecos de la anomalia no comenzo bloqueada: {initial:?}"
        ));
    }
    run_command(
        &mut session,
        "desbloquear Ecos de la anomalia",
        GameplayCommand::UnlockMission {
            mission: ecos_anomalia::ECOS_DE_LA_ANOMALIA,
        },
    )?;
    run_command(
        &mut session,
        "iniciar Ecos de la anomalia",
        GameplayCommand::StartMission {
            mission: ecos_anomalia::ECOS_DE_LA_ANOMALIA,
        },
    )?;
    run_command(
        &mut session,
        "ocupar pilotaje de la anomalia",
        GameplayCommand::Interact {
            player,
            target: pilot_station,
        },
    )?;
    run_command(
        &mut session,
        "detectar senal de la anomalia",
        GameplayCommand::ScanSignals { player, ship },
    )?;
    let after_detection = session
        .mission_progress(ecos_anomalia::ECOS_DE_LA_ANOMALIA)
        .ok_or_else(|| "se perdio progreso al detectar la senal".to_owned())?;
    if after_detection.required_completed != 0
        || session
            .world()
            .navigation()
            .signal_knowledge(ecos_anomalia::SIGNAL_DESTINATION)
            != SignalKnowledge::Detected
    {
        return Err("detectar la senal avanzo indebidamente la mision".to_owned());
    }
    run_command(
        &mut session,
        "esperar recarga de la senal",
        GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        },
    )?;
    run_command(
        &mut session,
        "identificar senal de la anomalia",
        GameplayCommand::ScanSignals { player, ship },
    )?;
    let after_identification = session
        .mission_progress(ecos_anomalia::ECOS_DE_LA_ANOMALIA)
        .ok_or_else(|| "se perdio progreso al identificar la senal".to_owned())?;
    if after_identification.required_completed != 1
        || session
            .world()
            .navigation()
            .signal_knowledge(ecos_anomalia::SIGNAL_DESTINATION)
            != SignalKnowledge::Identified
    {
        return Err("identificar la senal no completo el primer objetivo".to_owned());
    }

    run_command(
        &mut session,
        "seleccionar destino de la anomalia",
        GameplayCommand::SelectDestination {
            player,
            pilot_station,
            destination: ecos_anomalia::PLANET_DESTINATION,
        },
    )?;
    run_command(
        &mut session,
        "iniciar viaje a la anomalia",
        GameplayCommand::StartTravel {
            player,
            pilot_station,
        },
    )?;
    run_command(
        &mut session,
        "llegar al destino de la anomalia",
        GameplayCommand::AdvanceTime {
            delta: Duration(100),
        },
    )?;
    let after_arrival = session
        .mission_progress(ecos_anomalia::ECOS_DE_LA_ANOMALIA)
        .ok_or_else(|| "se perdio progreso al descubrir el planeta".to_owned())?;
    if after_arrival.required_completed != 2
        || session
            .world()
            .destination(ecos_anomalia::PLANET_DESTINATION)
            .map(|destination| destination.discovery)
            != Some(DiscoveryState::Discovered)
    {
        return Err("la llegada no completo la investigacion del destino".to_owned());
    }

    let ammunition = gameplay::types::ItemId(2);
    run_command(
        &mut session,
        "preparar municion de la anomalia",
        GameplayCommand::AddInventoryItem {
            player,
            item: ammunition,
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "equipar arma de la anomalia",
        GameplayCommand::EquipPlayerWeapon {
            player,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                60,
                20,
                Duration(1_000),
                WeaponAmmunition {
                    item: ammunition,
                    amount_per_attack: 1,
                },
            ),
        },
    )?;
    run_command(
        &mut session,
        "crear amenaza de la anomalia",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let enemy = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "la amenaza no publico su evento".to_owned())?;
    run_command(
        &mut session,
        "neutralizar amenaza de la anomalia",
        GameplayCommand::AttackEnemy { player, enemy },
    )?;
    let after_combat = session
        .mission_progress(ecos_anomalia::ECOS_DE_LA_ANOMALIA)
        .ok_or_else(|| "se perdio progreso al neutralizar la amenaza".to_owned())?;
    if after_combat.required_completed != 3
        || session
            .world()
            .combatant(enemy)
            .is_none_or(|combatant| combatant.active || combatant.current_health != 0)
    {
        return Err("la amenaza no fue neutralizada por el combate".to_owned());
    }
    let reward = session
        .world()
        .collectibles()
        .find(|collectible| collectible.item == gameplay::types::ItemId(3))
        .copied()
        .ok_or_else(|| "la amenaza no genero informacion recuperable".to_owned())?;
    run_command(
        &mut session,
        "recuperar informacion de la anomalia",
        GameplayCommand::CollectItem {
            player,
            collectible: reward.entity,
            amount: 1,
        },
    )?;
    let final_progress = session
        .mission_progress(ecos_anomalia::ECOS_DE_LA_ANOMALIA)
        .ok_or_else(|| "no existe progreso final de Ecos de la anomalia".to_owned())?;
    if final_progress.state != MissionState::Completed
        || final_progress.required_completed != final_progress.required_total
        || session
            .world()
            .inventory(player)
            .is_none_or(|inventory| inventory.quantity(gameplay::types::ItemId(3)) != 1)
    {
        return Err(format!(
            "Ecos de la anomalia no completo sus requisitos: {final_progress:?}"
        ));
    }
    println!(
        "MISION COMPLETADA: nombre=Ecos de la anomalia, obligatorios={}/{}, enemigo={}, informacion=1",
        final_progress.required_completed, final_progress.required_total, enemy
    );
    Ok(())
}

fn run_gravitational_storm_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let ship = session.ids.first_ship;
    let engineering_station = session
        .create_station(Vec3::ZERO, 2.0, StationType::Engineering)
        .map_err(|error| format!("crear estacion de ingenieria: {error:?}"))?;
    let pilot_station = session
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estacion de pilotaje: {error:?}"))?;
    session
        .register_destination(Destination {
            id: tormenta_gravitacional::ESCAPE_DESTINATION,
            destination_type: DestinationType::Planet,
            position: Vec3 {
                x: 10.0,
                y: 0.0,
                z: 0.0,
            },
            travel_duration: Duration(100),
            fuel_cost: 5,
            discovery: DiscoveryState::Known,
        })
        .map_err(|error| format!("registrar destino de escape: {error:?}"))?;
    run_command(
        &mut session,
        "registrar tormenta gravitacional",
        GameplayCommand::RegisterEnvironmentalHazard {
            hazard: EnvironmentalHazard::new(
                tormenta_gravitacional::HAZARD,
                gameplay::EnvironmentalHazardType::GravitationalAnomaly,
                Vec3::ZERO,
                5.0,
                80,
                Duration(1_000),
                true,
            ),
        },
    )?;

    let initial_integrity = session
        .world()
        .ship_integrity(ship)
        .ok_or_else(|| "la integridad inicial no esta disponible".to_owned())?;
    if initial_integrity.current() != 100 {
        return Err(format!(
            "integridad inicial inesperada: {}",
            initial_integrity.current()
        ));
    }
    let initial_mission = session
        .mission_progress(tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
        .ok_or_else(|| "no existe la mision Tormenta gravitacional".to_owned())?;
    if initial_mission.state != MissionState::Locked {
        return Err(format!(
            "la tormenta no comenzo bloqueada: {initial_mission:?}"
        ));
    }
    run_command(
        &mut session,
        "desbloquear Tormenta gravitacional",
        GameplayCommand::UnlockMission {
            mission: tormenta_gravitacional::TORMENTA_GRAVITACIONAL,
        },
    )?;
    run_command(
        &mut session,
        "iniciar Tormenta gravitacional",
        GameplayCommand::StartMission {
            mission: tormenta_gravitacional::TORMENTA_GRAVITACIONAL,
        },
    )?;
    let started_mission = session
        .mission_progress(tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
        .ok_or_else(|| "se perdio el estado inicial de la tormenta".to_owned())?;
    if started_mission.required_completed != 0 {
        return Err("la tormenta avanzo antes de recibir dano".to_owned());
    }

    run_command(
        &mut session,
        "ocupar ingenieria antes de la tormenta",
        GameplayCommand::Interact {
            player,
            target: engineering_station,
        },
    )?;
    run_command(
        &mut session,
        "procesar dano de la tormenta",
        GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        },
    )?;
    let damaged_integrity = session
        .world()
        .ship_integrity(ship)
        .ok_or_else(|| "la integridad desaparecio tras el dano".to_owned())?;
    let damaged_mission = session
        .mission_progress(tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
        .ok_or_else(|| "se perdio progreso tras el dano".to_owned())?;
    if damaged_integrity.current() != 20
        || damaged_integrity.state() != gameplay::StructuralState::Critical
        || damaged_mission.required_completed != 1
        || damaged_mission.state == MissionState::Completed
    {
        return Err(format!(
            "el dano o su objetivo fueron incoherentes: integridad={}, progreso={damaged_mission:?}",
            damaged_integrity.current()
        ));
    }

    run_command(
        &mut session,
        "aportar material de reparacion",
        GameplayCommand::AddInventoryItem {
            player,
            item: gameplay::types::ItemId(1),
            amount: 1,
        },
    )?;
    let material_before = session
        .world()
        .inventory(player)
        .ok_or_else(|| "el inventario no esta disponible antes de reparar".to_owned())?
        .quantity(gameplay::types::ItemId(1));
    let integrity_before_repair = session
        .world()
        .ship_integrity(ship)
        .ok_or_else(|| "la integridad no esta disponible antes de reparar".to_owned())?
        .current();
    if material_before != 1 || integrity_before_repair != 20 {
        return Err("el estado previo a la reparacion no es el esperado".to_owned());
    }
    run_command(
        &mut session,
        "reparar nave bajo presion",
        GameplayCommand::RepairShipAtStation {
            player,
            ship,
            station: engineering_station,
        },
    )?;
    let repaired_integrity = session
        .world()
        .ship_integrity(ship)
        .ok_or_else(|| "la integridad no esta disponible despues de reparar".to_owned())?;
    let material_after = session
        .world()
        .inventory(player)
        .ok_or_else(|| "el inventario no esta disponible despues de reparar".to_owned())?
        .quantity(gameplay::types::ItemId(1));
    let repaired_mission = session
        .mission_progress(tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
        .ok_or_else(|| "se perdio progreso tras reparar".to_owned())?;
    if repaired_integrity.current() != 40
        || material_after != 0
        || repaired_mission.required_completed != 2
        || repaired_mission.state == MissionState::Completed
    {
        return Err(format!(
            "la reparacion o su objetivo fueron incoherentes: integridad={}, material={}, progreso={repaired_mission:?}",
            repaired_integrity.current(), material_after
        ));
    }

    run_command(
        &mut session,
        "liberar ingenieria",
        GameplayCommand::ReleaseStation {
            station: engineering_station,
            player,
        },
    )?;
    run_command(
        &mut session,
        "ocupar pilotaje despues de reparar",
        GameplayCommand::Interact {
            player,
            target: pilot_station,
        },
    )?;
    run_command(
        &mut session,
        "seleccionar escape de la tormenta",
        GameplayCommand::SelectDestination {
            player,
            pilot_station,
            destination: tormenta_gravitacional::ESCAPE_DESTINATION,
        },
    )?;
    run_command(
        &mut session,
        "continuar viaje tras reparar",
        GameplayCommand::StartTravel {
            player,
            pilot_station,
        },
    )?;
    run_command(
        &mut session,
        "llegar al escape de la tormenta",
        GameplayCommand::AdvanceTime {
            delta: Duration(100),
        },
    )?;
    let final_mission = session
        .mission_progress(tormenta_gravitacional::TORMENTA_GRAVITACIONAL)
        .ok_or_else(|| "no existe el progreso final de la tormenta".to_owned())?;
    let final_integrity = session
        .world()
        .ship_integrity(ship)
        .ok_or_else(|| "la integridad final no esta disponible".to_owned())?;
    if final_mission.state != MissionState::Completed
        || final_mission.required_completed != final_mission.required_total
        || final_integrity.current() != 40
        || session.world().ship_position()
            != Some(Vec3 {
                x: 10.0,
                y: 0.0,
                z: 0.0,
            })
    {
        return Err(format!(
            "Tormenta gravitacional no completo sus requisitos: progreso={final_mission:?}, integridad={}",
            final_integrity.current()
        ));
    }
    println!(
        "MISION COMPLETADA: nombre=Tormenta gravitacional, obligatorios={}/{}, integridad_inicial={}, integridad_danada={}, integridad_reparada={}, material_consumido=1",
        final_mission.required_completed,
        final_mission.required_total,
        initial_integrity.current(),
        damaged_integrity.current(),
        final_integrity.current()
    );
    Ok(())
}

fn run_shielded_enemy_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let mission = la_defensa_imposible::LA_DEFENSA_IMPOSIBLE;
    let initial = session
        .mission_progress(mission)
        .ok_or_else(|| "no existe La defensa imposible".to_owned())?;
    if initial.state != MissionState::Locked {
        return Err(format!("la defensa no comenzo bloqueada: {initial:?}"));
    }
    run_command(
        &mut session,
        "desbloquear La defensa imposible",
        GameplayCommand::UnlockMission { mission },
    )?;
    run_command(
        &mut session,
        "iniciar La defensa imposible",
        GameplayCommand::StartMission { mission },
    )?;
    run_command(
        &mut session,
        "crear enemigo escudado",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shielded, Vec3::ZERO),
        },
    )?;
    let enemy = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated {
                enemy, archetype, ..
            } if *archetype == EnemyArchetype::Shielded => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "el enemigo escudado no publico su evento".to_owned())?;
    let initial_combatant = session
        .world()
        .combatant(enemy)
        .ok_or_else(|| "el enemigo escudado no tiene combatiente".to_owned())?;
    if initial_combatant.current_health != 100
        || initial_combatant
            .shield
            .is_none_or(|shield| shield.current != 50)
    {
        return Err("el estado inicial del escudo no es el esperado".to_owned());
    }
    if session
        .mission_progress(mission)
        .is_none_or(|progress| progress.required_completed != 0)
    {
        return Err("la mision avanzo al crear el enemigo".to_owned());
    }

    let kinetic_ammunition = gameplay::types::ItemId(2);
    run_command(
        &mut session,
        "preparar municion cinetica",
        GameplayCommand::AddInventoryItem {
            player,
            item: kinetic_ammunition,
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "equipar arma cinetica",
        GameplayCommand::EquipPlayerWeapon {
            player,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                10,
                20,
                Duration(1_000),
                WeaponAmmunition {
                    item: kinetic_ammunition,
                    amount_per_attack: 1,
                },
            ),
        },
    )?;
    run_command(
        &mut session,
        "probar defensa con arma cinetica",
        GameplayCommand::AttackEnemy { player, enemy },
    )?;
    let after_wrong_attack = session
        .world()
        .combatant(enemy)
        .ok_or_else(|| "desaparecio el enemigo tras el ataque cinetico".to_owned())?;
    let after_wrong_mission = session
        .mission_progress(mission)
        .ok_or_else(|| "se perdio progreso tras probar la defensa".to_owned())?;
    if after_wrong_attack.current_health != 100
        || after_wrong_attack.active == false
        || after_wrong_attack
            .shield
            .is_none_or(|shield| shield.current != 40)
        || session
            .world()
            .inventory(player)
            .is_none_or(|inventory| inventory.quantity(kinetic_ammunition) != 0)
        || after_wrong_mission.required_completed != 1
        || after_wrong_mission.state == MissionState::Completed
    {
        return Err(format!(
            "el arma inadecuada no respeto el escudo: enemigo={after_wrong_attack:?}, progreso={after_wrong_mission:?}"
        ));
    }

    let energy_ammunition = gameplay::types::ItemId(3);
    run_command(
        &mut session,
        "preparar municion de energia",
        GameplayCommand::AddInventoryItem {
            player,
            item: energy_ammunition,
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "equipar arma de energia",
        GameplayCommand::EquipPlayerWeapon {
            player,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Energy,
                100,
                20,
                Duration(1_000),
                WeaponAmmunition {
                    item: energy_ammunition,
                    amount_per_attack: 1,
                },
            ),
        },
    )?;
    run_command(
        &mut session,
        "esperar enfriamiento del arma especial",
        GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        },
    )?;
    run_command(
        &mut session,
        "desactivar escudo con energia",
        GameplayCommand::AttackEnemy { player, enemy },
    )?;
    let after_energy_attack = session
        .world()
        .combatant(enemy)
        .ok_or_else(|| "desaparecio el enemigo tras el ataque de energia".to_owned())?;
    let after_shield_mission = session
        .mission_progress(mission)
        .ok_or_else(|| "se perdio progreso al desactivar el escudo".to_owned())?;
    if after_energy_attack.current_health != 0
        || after_energy_attack.active
        || after_energy_attack
            .shield
            .is_none_or(|shield| shield.current != 0)
        || session
            .world()
            .inventory(player)
            .is_none_or(|inventory| inventory.quantity(energy_ammunition) != 0)
        || after_shield_mission.required_completed != 3
        || after_shield_mission.state == MissionState::Completed
    {
        return Err(format!(
            "el arma de energia no desactivo el escudo como se esperaba: enemigo={after_energy_attack:?}, progreso={after_shield_mission:?}"
        ));
    }
    let reward = session
        .world()
        .collectibles()
        .find(|collectible| collectible.item == gameplay::types::ItemId(1))
        .copied()
        .ok_or_else(|| "el escudado no genero recompensa".to_owned())?;
    if reward.amount != 1 || !reward.active {
        return Err("la recompensa del escudado tiene un estado invalido".to_owned());
    }
    run_command(
        &mut session,
        "recoger recompensa del escudado",
        GameplayCommand::CollectItem {
            player,
            collectible: reward.entity,
            amount: 1,
        },
    )?;
    let final_progress = session
        .mission_progress(mission)
        .ok_or_else(|| "no existe progreso final de La defensa imposible".to_owned())?;
    if final_progress.state != MissionState::Completed
        || final_progress.required_completed != final_progress.required_total
        || session
            .world()
            .inventory(player)
            .is_none_or(|inventory| inventory.quantity(gameplay::types::ItemId(1)) != 1)
        || session
            .world()
            .collectible(reward.entity)
            .is_none_or(|collectible| collectible.active || collectible.amount != 0)
    {
        return Err(format!(
            "La defensa imposible no completo sus requisitos: {final_progress:?}"
        ));
    }
    println!(
        "MISION COMPLETADA: nombre=La defensa imposible, obligatorios={}/{}, enemigo={}, municion_cinetica=1, municion_energia_consumida=1, recompensa_recogida=1",
        final_progress.required_completed, final_progress.required_total, enemy
    );
    Ok(())
}

fn run_cooperative_revival_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let helper = session.ids.first_player;
    let target = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador objetivo: {error:?}"))?;
    session
        .world_mut()
        .initialize_inventory(target, 20)
        .map_err(|error| format!("inicializar inventario del objetivo: {error:?}"))?;
    let revival_item = gameplay::types::ItemId(5);
    run_command(
        &mut session,
        "preparar consumibles de reanimacion",
        GameplayCommand::AddInventoryItem {
            player: helper,
            item: revival_item,
            amount: 2,
        },
    )?;
    let helper_initial = session
        .world()
        .player(helper)
        .ok_or_else(|| "no existe el jugador ayudante".to_owned())?;
    let target_initial = session
        .world()
        .player(target)
        .ok_or_else(|| "no existe el jugador objetivo".to_owned())?;
    if !helper_initial.is_active
        || !target_initial.is_active
        || !session.world().entities().contains(helper)
        || !session.world().entities().contains(target)
        || !session
            .world()
            .combatant(helper)
            .is_some_and(|value| value.active)
        || !session
            .world()
            .combatant(target)
            .is_some_and(|value| value.active)
    {
        return Err("los dos jugadores no comenzaron activos".to_owned());
    }

    let mut enemy_configuration =
        EnemyConfiguration::for_archetype(EnemyArchetype::Charger, Vec3::ZERO);
    enemy_configuration.weapon =
        WeaponConfiguration::new(AttackType::Kinetic, 100, 20, Duration(1));
    run_command(
        &mut session,
        "crear atacante para la derrota cooperativa",
        GameplayCommand::CreateEnemy {
            configuration: enemy_configuration,
        },
    )?;
    let enemy = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "no se creo el atacante cooperativo".to_owned())?;
    let target_health_before = session
        .world()
        .player_health(target)
        .ok_or_else(|| "no se pudo consultar la salud inicial del objetivo".to_owned())?;
    run_command(
        &mut session,
        "derrotar jugador objetivo mediante combate",
        GameplayCommand::EnemyAttackPlayer {
            enemy,
            player: target,
        },
    )?;
    let target_after_defeat = session
        .world()
        .player_health(target)
        .ok_or_else(|| "no se pudo consultar la salud tras la derrota".to_owned())?;
    let helper_after_defeat = session
        .world()
        .player(helper)
        .ok_or_else(|| "desaparecio el ayudante tras la derrota".to_owned())?;
    let target_combatant_after_defeat = session
        .world()
        .combatant(target)
        .ok_or_else(|| "desaparecio el combatant del objetivo".to_owned())?;
    if target_health_before.0 != 100
        || target_after_defeat.0 != 0
        || target_after_defeat.2
        || target_combatant_after_defeat.active
        || !target_combatant_after_defeat.is_defeated()
        || session.world().entities().contains(target)
        || !helper_after_defeat.is_active
        || !session.world().entities().contains(helper)
    {
        return Err(format!(
            "la derrota cooperativa dejo un estado incoherente: salud={target_after_defeat:?}, combatant={target_combatant_after_defeat:?}"
        ));
    }
    let helper_item_before = session
        .world()
        .inventory(helper)
        .ok_or_else(|| "el inventario del ayudante no existe".to_owned())?
        .quantity(revival_item);
    if helper_item_before != 2
        || !session
            .world()
            .can_player_revive(helper, target, revival_item)
    {
        return Err("el estado previo no cumple los requisitos de reanimacion".to_owned());
    }

    run_command(
        &mut session,
        "reanimar jugador objetivo",
        GameplayCommand::RevivePlayer {
            helper,
            target,
            item: revival_item,
        },
    )?;
    let target_after_revival = session
        .world()
        .player_health(target)
        .ok_or_else(|| "no se pudo consultar la salud tras reanimar".to_owned())?;
    let target_player_after_revival = session
        .world()
        .player(target)
        .ok_or_else(|| "no existe el jugador reanimado".to_owned())?;
    let target_combatant_after_revival = session
        .world()
        .combatant(target)
        .ok_or_else(|| "no existe el combatant reanimado".to_owned())?;
    let helper_item_after = session
        .world()
        .inventory(helper)
        .ok_or_else(|| "desaparecio el inventario del ayudante".to_owned())?
        .quantity(revival_item);
    if target_after_revival.0 != 30
        || !target_after_revival.2
        || !target_player_after_revival.is_active
        || !target_combatant_after_revival.active
        || target_combatant_after_revival.current_health != 30
        || target_combatant_after_revival.is_defeated()
        || !session.world().entities().contains(target)
        || helper_item_after != 1
    {
        return Err(format!(
            "la reanimacion no restauro coherentemente el objetivo: salud={target_after_revival:?}, item={helper_item_after}"
        ));
    }
    if session
        .world()
        .player(target)
        .map(|player| player.entity_id)
        != Some(target)
        || session
            .world()
            .combatant(target)
            .map(|combatant| combatant.entity)
            != Some(target)
    {
        return Err("la reanimacion sustituyo la identidad del objetivo".to_owned());
    }

    let invalid_result = session
        .update([GameplayCommand::RevivePlayer {
            helper,
            target,
            item: revival_item,
        }])
        .into_iter()
        .next()
        .ok_or_else(|| "la segunda reanimacion no devolvio resultado".to_owned())?;
    if !matches!(
        invalid_result.status,
        gameplay::CommandStatus::Rejected {
            reason: gameplay::CommandRejection::ReviveTargetNotDefeated
        }
    ) || session
        .world()
        .inventory(helper)
        .is_none_or(|inventory| inventory.quantity(revival_item) != 1)
        || session
            .world()
            .player_health(target)
            .is_none_or(|health| health.0 != 30)
        || !session.world().entities().contains(target)
    {
        return Err(format!(
            "la reanimacion invalida no fue atomica: resultado={:?}",
            invalid_result.status
        ));
    }
    println!(
        "COOPERACION COMPLETADA: ayudante={}, objetivo={}, salud={}, consumible_restante={}",
        helper, target, target_after_revival.0, helper_item_after
    );
    Ok(())
}

fn run_cooperative_combat_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player_a = session.ids.first_player;
    let player_b = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador B: {error:?}"))?;
    session
        .world_mut()
        .initialize_inventory(player_b, 20)
        .map_err(|error| format!("inicializar inventario del jugador B: {error:?}"))?;
    if player_a == player_b
        || !session.world().entities().contains(player_a)
        || !session.world().entities().contains(player_b)
        || !session
            .world()
            .player(player_a)
            .is_some_and(|player| player.is_active)
        || !session
            .world()
            .player(player_b)
            .is_some_and(|player| player.is_active)
    {
        return Err("los dos jugadores cooperativos no son entidades activas distintas".to_owned());
    }
    let ammunition = gameplay::types::ItemId(2);
    run_command(
        &mut session,
        "dar municion al jugador A",
        GameplayCommand::AddInventoryItem {
            player: player_a,
            item: ammunition,
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "dar municion al jugador B",
        GameplayCommand::AddInventoryItem {
            player: player_b,
            item: ammunition,
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "equipar arma del jugador A",
        GameplayCommand::EquipPlayerWeapon {
            player: player_a,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                40,
                20,
                Duration(1_000),
                WeaponAmmunition {
                    item: ammunition,
                    amount_per_attack: 1,
                },
            ),
        },
    )?;
    run_command(
        &mut session,
        "equipar arma del jugador B",
        GameplayCommand::EquipPlayerWeapon {
            player: player_b,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                60,
                20,
                Duration(1_000),
                WeaponAmmunition {
                    item: ammunition,
                    amount_per_attack: 1,
                },
            ),
        },
    )?;
    let mut enemy_configuration =
        EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO);
    enemy_configuration.maximum_health = 100;
    run_command(
        &mut session,
        "crear enemigo del combate cooperativo",
        GameplayCommand::CreateEnemy {
            configuration: enemy_configuration,
        },
    )?;
    let enemy = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "el enemigo cooperativo no publico su evento".to_owned())?;
    let initial_health = session
        .world()
        .combatant(enemy)
        .ok_or_else(|| "el enemigo cooperativo no tiene combatiente".to_owned())?
        .current_health;
    let ammunition_a_before = session
        .world()
        .inventory(player_a)
        .ok_or_else(|| "no existe el inventario del jugador A".to_owned())?
        .quantity(ammunition);
    let ammunition_b_before = session
        .world()
        .inventory(player_b)
        .ok_or_else(|| "no existe el inventario del jugador B".to_owned())?
        .quantity(ammunition);
    if initial_health != 100 || ammunition_a_before != 1 || ammunition_b_before != 1 {
        return Err("el estado inicial del combate cooperativo es incoherente".to_owned());
    }

    run_command(
        &mut session,
        "atacar enemigo con jugador A",
        GameplayCommand::AttackEnemy {
            player: player_a,
            enemy,
        },
    )?;
    let after_a = session
        .world()
        .combatant(enemy)
        .ok_or_else(|| "desaparecio el enemigo despues del ataque A".to_owned())?;
    let attack_a = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::CombatDamageApplied {
                attacker,
                target,
                amount,
                remaining_health,
            } if *attacker == player_a && *target == enemy => Some((*amount, *remaining_health)),
            _ => None,
        });
    let ammunition_a_after = session
        .world()
        .inventory(player_a)
        .ok_or_else(|| "desaparecio el inventario A despues de atacar".to_owned())?
        .quantity(ammunition);
    let ammunition_b_after_a = session
        .world()
        .inventory(player_b)
        .ok_or_else(|| "desaparecio el inventario B despues del ataque A".to_owned())?
        .quantity(ammunition);
    if attack_a != Some((40, 60))
        || after_a.current_health != 60
        || !after_a.active
        || ammunition_a_after != 0
        || ammunition_b_after_a != 1
    {
        return Err(format!(
            "el ataque A no produjo una contribucion parcial valida: evento={attack_a:?}, enemigo={after_a:?}, municion_A={ammunition_a_after}, municion_B={ammunition_b_after_a}"
        ));
    }
    if session
        .world()
        .combatant(player_a)
        .is_none_or(|combatant| combatant.current_health != 100 || !combatant.active)
        || session
            .world()
            .combatant(player_b)
            .is_none_or(|combatant| combatant.current_health != 100 || !combatant.active)
    {
        return Err("el ataque A altero indebidamente la salud de otro jugador".to_owned());
    }

    run_command(
        &mut session,
        "atacar enemigo con jugador B",
        GameplayCommand::AttackEnemy {
            player: player_b,
            enemy,
        },
    )?;
    let after_b = session
        .world()
        .combatant(enemy)
        .ok_or_else(|| "desaparecio el enemigo despues del ataque B".to_owned())?;
    let attack_b = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::CombatDamageApplied {
                attacker,
                target,
                amount,
                remaining_health,
            } if *attacker == player_b && *target == enemy => Some((*amount, *remaining_health)),
            _ => None,
        });
    let ammunition_a_after_b = session
        .world()
        .inventory(player_a)
        .ok_or_else(|| "desaparecio el inventario A despues del ataque B".to_owned())?
        .quantity(ammunition);
    let ammunition_b_after = session
        .world()
        .inventory(player_b)
        .ok_or_else(|| "desaparecio el inventario B despues de atacar".to_owned())?
        .quantity(ammunition);
    if attack_b != Some((60, 0))
        || after_b.current_health != 0
        || after_b.active
        || ammunition_a_after_b != 0
        || ammunition_b_after != 0
        || !session.world().entities().contains(enemy)
    {
        return Err(format!(
            "el ataque B no completo correctamente el combate: evento={attack_b:?}, enemigo={after_b:?}, municion_A={ammunition_a_after_b}, municion_B={ammunition_b_after}"
        ));
    }
    let reward = session
        .world()
        .collectibles()
        .find(|collectible| collectible.item == gameplay::types::ItemId(3))
        .copied()
        .ok_or_else(|| "la derrota cooperativa no genero recompensa".to_owned())?;
    if reward.amount != 1 || !reward.active {
        return Err("la recompensa cooperativa tiene un estado invalido".to_owned());
    }
    run_command(
        &mut session,
        "recoger recompensa con jugador B",
        GameplayCommand::CollectItem {
            player: player_b,
            collectible: reward.entity,
            amount: 1,
        },
    )?;
    if session
        .world()
        .inventory(player_b)
        .is_none_or(|inventory| inventory.quantity(gameplay::types::ItemId(3)) != 1)
        || session
            .world()
            .collectible(reward.entity)
            .is_none_or(|collectible| collectible.active || collectible.amount != 0)
    {
        return Err("la recoleccion cooperativa no actualizo el estado esperado".to_owned());
    }
    println!(
        "COMBATE COOPERATIVO COMPLETADO: jugador_A={}, jugador_B={}, enemigo={}, salud_inicial={}, dano_A=40, salud_intermedia=60, dano_B=60, municion_A=0, municion_B=0, recompensa_recogida=1",
        player_a, player_b, enemy, initial_health
    );
    Ok(())
}

fn run_cooperative_progression_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player_a = session.ids.first_player;
    let ship = session.ids.first_ship;
    let player_b = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador B: {error:?}"))?;
    session
        .world_mut()
        .initialize_inventory(player_b, 20)
        .map_err(|error| format!("inicializar inventario del jugador B: {error:?}"))?;
    if player_a == player_b
        || !session.world().entities().contains(player_a)
        || !session.world().entities().contains(player_b)
        || !session
            .world()
            .player(player_a)
            .is_some_and(|player| player.is_active)
        || !session
            .world()
            .player(player_b)
            .is_some_and(|player| player.is_active)
    {
        return Err("los jugadores de la progresion no son entidades activas distintas".to_owned());
    }
    session
        .register_mission(
            cooperacion::definition()
                .map_err(|error| format!("definir mision cooperativa: {error:?}"))?,
        )
        .map_err(|error| format!("registrar mision cooperativa: {error:?}"))?;
    session
        .register_destination(Destination {
            id: cooperacion::SIGNAL_DESTINATION,
            destination_type: DestinationType::SpacePoint,
            position: Vec3::ZERO,
            travel_duration: Duration(100),
            fuel_cost: 1,
            discovery: DiscoveryState::Unknown,
        })
        .map_err(|error| format!("registrar senal cooperativa: {error:?}"))?;
    let station = session
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estacion cooperativa: {error:?}"))?;
    let objective_states = |session: &ProjectEchoSession| {
        session
            .world()
            .mission(cooperacion::PROGRESION_COOPERATIVA)
            .and_then(|mission| {
                Some((
                    mission
                        .objectives()
                        .iter()
                        .find(|objective| objective.id == cooperacion::IDENTIFICAR_SENAL)?
                        .state(),
                    mission
                        .objectives()
                        .iter()
                        .find(|objective| objective.id == cooperacion::DERROTAR_AMENAZA)?
                        .state(),
                    mission
                        .objectives()
                        .iter()
                        .find(|objective| objective.id == cooperacion::RECUPERAR_RECOMPENSA)?
                        .state(),
                ))
            })
    };

    let initial = session
        .mission_progress(cooperacion::PROGRESION_COOPERATIVA)
        .ok_or_else(|| "no existe la mision cooperativa".to_owned())?;
    if initial.state != MissionState::Locked
        || initial.required_completed != 0
        || initial.required_total != 3
    {
        return Err(format!(
            "estado inicial cooperativo inesperado: {initial:?}"
        ));
    }
    run_command(
        &mut session,
        "desbloquear mision cooperativa",
        GameplayCommand::UnlockMission {
            mission: cooperacion::PROGRESION_COOPERATIVA,
        },
    )?;
    run_command(
        &mut session,
        "iniciar mision cooperativa",
        GameplayCommand::StartMission {
            mission: cooperacion::PROGRESION_COOPERATIVA,
        },
    )?;
    let active = session
        .mission_progress(cooperacion::PROGRESION_COOPERATIVA)
        .ok_or_else(|| "se perdio la mision cooperativa al iniciarla".to_owned())?;
    if active.state != MissionState::Active
        || active.required_completed != 0
        || objective_states(&session)
            != Some((
                ObjectiveState::Available,
                ObjectiveState::Locked,
                ObjectiveState::Locked,
            ))
    {
        return Err(format!(
            "la mision cooperativa no inicio correctamente: {active:?}"
        ));
    }

    run_command(
        &mut session,
        "ocupar estacion con jugador A",
        GameplayCommand::Interact {
            player: player_a,
            target: station,
        },
    )?;
    run_command(
        &mut session,
        "detectar senal con jugador A",
        GameplayCommand::ScanSignals {
            player: player_a,
            ship,
        },
    )?;
    let after_detection = session
        .mission_progress(cooperacion::PROGRESION_COOPERATIVA)
        .ok_or_else(|| "se perdio el progreso tras detectar la senal".to_owned())?;
    if after_detection.state != MissionState::Active
        || after_detection.required_completed != 0
        || session
            .world()
            .navigation()
            .signal_knowledge(cooperacion::SIGNAL_DESTINATION)
            != SignalKnowledge::Detected
    {
        return Err("la deteccion avanzo indebidamente la mision cooperativa".to_owned());
    }
    run_command(
        &mut session,
        "esperar recarga cooperativa",
        GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        },
    )?;
    run_command(
        &mut session,
        "identificar senal con jugador A",
        GameplayCommand::ScanSignals {
            player: player_a,
            ship,
        },
    )?;
    let after_identification = session
        .mission_progress(cooperacion::PROGRESION_COOPERATIVA)
        .ok_or_else(|| "se perdio el progreso tras identificar la senal".to_owned())?;
    if after_identification.state != MissionState::Active
        || after_identification.required_completed != 1
        || objective_states(&session)
            != Some((
                ObjectiveState::Completed,
                ObjectiveState::Available,
                ObjectiveState::Locked,
            ))
        || session
            .world()
            .navigation()
            .signal_knowledge(cooperacion::SIGNAL_DESTINATION)
            != SignalKnowledge::Identified
    {
        return Err("el objetivo de identificacion no avanzo con el jugador A".to_owned());
    }

    let ammunition = gameplay::types::ItemId(2);
    run_command(
        &mut session,
        "dar municion al jugador B",
        GameplayCommand::AddInventoryItem {
            player: player_b,
            item: ammunition,
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "equipar arma del jugador B",
        GameplayCommand::EquipPlayerWeapon {
            player: player_b,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                60,
                20,
                Duration(1_000),
                WeaponAmmunition {
                    item: ammunition,
                    amount_per_attack: 1,
                },
            ),
        },
    )?;
    run_command(
        &mut session,
        "crear amenaza cooperativa",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let enemy = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "la amenaza cooperativa no publico su evento".to_owned())?;
    run_command(
        &mut session,
        "derrotar amenaza con jugador B",
        GameplayCommand::AttackEnemy {
            player: player_b,
            enemy,
        },
    )?;
    let defeat_event = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyDefeated {
                enemy: defeated,
                archetype,
            } if *defeated == enemy => Some(*archetype),
            _ => None,
        });
    let damage_by_b = session.world().events().iter().any(|event| {
        matches!(
            event,
            GameplayEvent::CombatDamageApplied {
                attacker,
                target,
                amount: 60,
                remaining_health: 0,
            } if *attacker == player_b && *target == enemy
        )
    });
    let after_defeat = session
        .mission_progress(cooperacion::PROGRESION_COOPERATIVA)
        .ok_or_else(|| "se perdio el progreso tras la derrota cooperativa".to_owned())?;
    if defeat_event != Some(EnemyArchetype::Shooter)
        || !damage_by_b
        || after_defeat.state != MissionState::Active
        || after_defeat.required_completed != 2
        || objective_states(&session)
            != Some((
                ObjectiveState::Completed,
                ObjectiveState::Completed,
                ObjectiveState::Available,
            ))
        || session
            .world()
            .combatant(enemy)
            .is_none_or(|combatant| combatant.active || combatant.current_health != 0)
    {
        return Err("el jugador B no completo el segundo objetivo cooperativo".to_owned());
    }

    let reward = session
        .world()
        .collectibles()
        .find(|collectible| collectible.item == gameplay::types::ItemId(3))
        .copied()
        .ok_or_else(|| "la amenaza no genero la recompensa cooperativa".to_owned())?;
    let reward_before = session
        .world()
        .inventory(player_a)
        .ok_or_else(|| "no existe el inventario del jugador A".to_owned())?
        .quantity(gameplay::types::ItemId(3));
    run_command(
        &mut session,
        "recoger recompensa con jugador A",
        GameplayCommand::CollectItem {
            player: player_a,
            collectible: reward.entity,
            amount: 1,
        },
    )?;
    let final_progress = session
        .mission_progress(cooperacion::PROGRESION_COOPERATIVA)
        .ok_or_else(|| "no existe el progreso final cooperativo".to_owned())?;
    let reward_after = session
        .world()
        .inventory(player_a)
        .ok_or_else(|| "desaparecio el inventario final del jugador A".to_owned())?
        .quantity(gameplay::types::ItemId(3));
    if final_progress.state != MissionState::Completed
        || final_progress.required_completed != final_progress.required_total
        || objective_states(&session)
            != Some((
                ObjectiveState::Completed,
                ObjectiveState::Completed,
                ObjectiveState::Completed,
            ))
        || reward_after != reward_before + 1
        || session
            .world()
            .collectible(reward.entity)
            .is_none_or(|collectible| collectible.active || collectible.amount != 0)
    {
        return Err(format!(
            "la mision cooperativa no completo su cadena: {final_progress:?}, recompensa_antes={reward_before}, despues={reward_after}"
        ));
    }
    println!(
        "PROGRESION COOPERATIVA COMPLETADA: mision={:?}, jugador_A={}, jugador_B={}, enemigo={}, progreso={}/{}, recompensa_recogida=1",
        cooperacion::PROGRESION_COOPERATIVA,
        player_a,
        player_b,
        enemy,
        final_progress.required_completed,
        final_progress.required_total
    );
    Ok(())
}

fn run_player_count_scenario(player_count: usize) -> Result<(), String> {
    if !(1..=gameplay::MAX_LOCAL_PLAYERS).contains(&player_count) {
        return Err(format!("cantidad de jugadores invalida: {player_count}"));
    }
    let mut session = ProjectEchoSession::new();
    let mut players = vec![session.ids.first_player];
    let ship = session.ids.first_ship;
    while players.len() < player_count {
        let player = session
            .world_mut()
            .create_player()
            .map_err(|error| format!("crear jugador {player_count}: {error:?}"))?;
        session
            .world_mut()
            .initialize_inventory(player, 20)
            .map_err(|error| format!("inicializar inventario de jugador {player}: {error:?}"))?;
        players.push(player);
    }
    if players.iter().enumerate().any(|(index, player)| {
        players[..index].contains(player)
            || !session.world().entities().contains(*player)
            || session.world().player(*player).is_none()
            || session.world().combatant(*player).is_none_or(|combatant| !combatant.is_player)
            || session.world().inventory(*player).is_none()
    }) || session.world().players().count() != player_count
    {
        return Err(format!(
            "la sesion de {player_count} jugadores no registro componentes independientes"
        ));
    }

    let ammunition = gameplay::types::ItemId(2);
    for player in players.iter().copied() {
        run_command(
            &mut session,
            "dar municion de completabilidad",
            GameplayCommand::AddInventoryItem {
                player,
                item: ammunition,
                amount: 1,
            },
        )?;
    }
    if players.iter().any(|player| {
        session
            .world()
            .inventory(*player)
            .is_none_or(|inventory| inventory.quantity(ammunition) != 1)
    }) {
        return Err(format!(
            "los inventarios de la sesion de {player_count} jugadores no son independientes"
        ));
    }

    if player_count == gameplay::MAX_LOCAL_PLAYERS {
        let players_before = session.world().players().count();
        let active_before = session.world().entities().active_count();
        let events_before = session.world().events().len();
        let rejected = session.world_mut().create_player();
        if rejected != Err(gameplay::PlayerCreationError::MaximumPlayersReached)
            || session.world().players().count() != players_before
            || session.world().entities().active_count() != active_before
            || session.world().events().len() != events_before
        {
            return Err("crear el cuarto jugador no fue rechazado atomica y tipadamente".to_owned());
        }
    }

    session
        .register_mission(
            cooperacion::definition()
                .map_err(|error| format!("definir mision de {player_count} jugadores: {error:?}"))?,
        )
        .map_err(|error| format!("registrar mision de {player_count} jugadores: {error:?}"))?;
    session
        .register_destination(Destination {
            id: cooperacion::SIGNAL_DESTINATION,
            destination_type: DestinationType::SpacePoint,
            position: Vec3::ZERO,
            travel_duration: Duration(100),
            fuel_cost: 1,
            discovery: DiscoveryState::Unknown,
        })
        .map_err(|error| format!("registrar senal de {player_count} jugadores: {error:?}"))?;
    let station = session
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estacion de {player_count} jugadores: {error:?}"))?;
    run_command(
        &mut session,
        "desbloquear mision de completabilidad",
        GameplayCommand::UnlockMission {
            mission: cooperacion::PROGRESION_COOPERATIVA,
        },
    )?;
    run_command(
        &mut session,
        "iniciar mision de completabilidad",
        GameplayCommand::StartMission {
            mission: cooperacion::PROGRESION_COOPERATIVA,
        },
    )?;
    run_command(
        &mut session,
        "ocupar estacion de completabilidad",
        GameplayCommand::Interact {
            player: players[0],
            target: station,
        },
    )?;
    run_command(
        &mut session,
        "detectar senal de completabilidad",
        GameplayCommand::ScanSignals {
            player: players[0],
            ship,
        },
    )?;
    run_command(
        &mut session,
        "recargar escaner de completabilidad",
        GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        },
    )?;
    run_command(
        &mut session,
        "identificar senal de completabilidad",
        GameplayCommand::ScanSignals {
            player: players[0],
            ship,
        },
    )?;
    let after_signal = session
        .mission_progress(cooperacion::PROGRESION_COOPERATIVA)
        .ok_or_else(|| "se perdio la mision de completabilidad tras la senal".to_owned())?;
    if after_signal.state != MissionState::Active || after_signal.required_completed != 1 {
        return Err(format!(
            "la sesion de {player_count} jugadores no completo el objetivo de senal: {after_signal:?}"
        ));
    }
    if player_count > 1 {
        run_command(
            &mut session,
            "desactivar jugador A despues de la senal",
            GameplayCommand::SetPlayerActive {
                player: players[0],
                active: false,
            },
        )?;
        if session
            .world()
            .player(players[0])
            .is_none_or(|player| player.is_active)
            || session
                .mission_progress(cooperacion::PROGRESION_COOPERATIVA)
                .is_none_or(|progress| progress.required_completed != 1)
        {
            return Err("desactivar un jugador borro la progresion cooperativa".to_owned());
        }
    }

    let attacker = players.get(1).copied().unwrap_or(players[0]);
    run_command(
        &mut session,
        "equipar arma de completabilidad",
        GameplayCommand::EquipPlayerWeapon {
            player: attacker,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                60,
                20,
                Duration(1_000),
                WeaponAmmunition {
                    item: ammunition,
                    amount_per_attack: 1,
                },
            ),
        },
    )?;
    run_command(
        &mut session,
        "crear enemigo de completabilidad",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let enemy = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "el enemigo de completabilidad no fue creado".to_owned())?;
    run_command(
        &mut session,
        "derrotar enemigo de completabilidad",
        GameplayCommand::AttackEnemy { player: attacker, enemy },
    )?;
    let after_defeat = session
        .mission_progress(cooperacion::PROGRESION_COOPERATIVA)
        .ok_or_else(|| "se perdio la mision tras la derrota de completabilidad".to_owned())?;
    let ammunition_isolated = players.iter().all(|player| {
        session
            .world()
            .inventory(*player)
            .is_some_and(|inventory| inventory.quantity(ammunition) == u32::from(*player != attacker))
    });
    if after_defeat.state != MissionState::Active
        || after_defeat.required_completed != 2
        || !ammunition_isolated
    {
        return Err(format!(
            "la sesion de {player_count} jugadores no completo el objetivo de combate: {after_defeat:?}"
        ));
    }
    let collector = players[player_count - 1];
    let reward = session
        .world()
        .collectibles()
        .find(|collectible| collectible.item == gameplay::types::ItemId(3))
        .copied()
        .ok_or_else(|| "no se genero la recompensa de completabilidad".to_owned())?;
    run_command(
        &mut session,
        "recoger recompensa de completabilidad",
        GameplayCommand::CollectItem {
            player: collector,
            collectible: reward.entity,
            amount: 1,
        },
    )?;
    let final_progress = session
        .mission_progress(cooperacion::PROGRESION_COOPERATIVA)
        .ok_or_else(|| "no existe el progreso final de completabilidad".to_owned())?;
    if final_progress.state != MissionState::Completed
        || final_progress.required_completed != final_progress.required_total
        || players.iter().any(|player| {
            session
                .world()
                .inventory(*player)
                .is_none_or(|inventory| {
                    inventory.quantity(gameplay::types::ItemId(3))
                        != u32::from(*player == collector)
                })
        })
        || session
            .world()
            .inventory(collector)
            .is_none_or(|inventory| inventory.quantity(gameplay::types::ItemId(3)) != 1)
        || session
            .world()
            .collectible(reward.entity)
            .is_none_or(|collectible| collectible.active || collectible.amount != 0)
    {
        return Err(format!(
            "la sesion de {player_count} jugadores no completo la mision: {final_progress:?}"
        ));
    }
    println!(
        "COMPLETABILIDAD {} JUGADOR(ES) COMPLETADA: mision={:?}, progreso={}/{}, atacante={}, recolector={}",
        player_count,
        cooperacion::PROGRESION_COOPERATIVA,
        final_progress.required_completed,
        final_progress.required_total,
        attacker,
        collector
    );
    Ok(())
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

fn run_erick_cooperative_support_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let needy = session.ids.first_player;
    let provider_a = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear proveedor A: {error:?}"))?;
    let provider_b = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear proveedor B: {error:?}"))?;
    for provider in [provider_a, provider_b] {
        session
            .world_mut()
            .initialize_inventory(provider, 20)
            .map_err(|error| format!("inicializar inventario de proveedor: {error:?}"))?;
    }
    run_command(
        &mut session,
        "crear ERICK para apoyo cooperativo",
        GameplayCommand::CreateCompanion {
            followed_player: needy,
            position: Vec3::ZERO,
        },
    )?;
    let erick = session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK de apoyo no quedo registrado".to_owned())?;
    run_command(
        &mut session,
        "equipar arma con bloqueo de municion",
        GameplayCommand::EquipPlayerWeapon {
            player: needy,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                20,
                20,
                Duration(1),
                WeaponAmmunition {
                    item: ItemId(2),
                    amount_per_attack: 2,
                },
            ),
        },
    )?;
    run_command(
        &mut session,
        "entregar municion al primer proveedor",
        GameplayCommand::AddInventoryItem {
            player: provider_a,
            item: ItemId(2),
            amount: 5,
        },
    )?;
    run_command(
        &mut session,
        "entregar municion al segundo proveedor",
        GameplayCommand::AddInventoryItem {
            player: provider_b,
            item: ItemId(2),
            amount: 5,
        },
    )?;
    run_command(
        &mut session,
        "crear amenaza para bloqueo de preparacion",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let enemy = session
        .world()
        .enemies()
        .next()
        .map(|enemy| enemy.entity)
        .ok_or_else(|| "amenaza de apoyo no quedo registrada".to_owned())?;
    let before_queries = session.world().save_state().serialize();
    let support = session
        .world()
        .companion_cooperative_support_assessment(erick.entity)
        .map_err(|error| format!("evaluar apoyo cooperativo: {error:?}"))?
        .ok_or_else(|| "la evaluacion de apoyo no estuvo disponible".to_owned())?;
    let recommendation = support
        .recommendations
        .first()
        .ok_or_else(|| "no se detecto el bloqueo de municion".to_owned())?;
    if support.status != gameplay::SupportRecommendationStatus::Actionable
        || recommendation.needy_player != needy
        || recommendation.provider != Some(provider_a)
        || recommendation.item != Some(ItemId(2))
        || recommendation.amount != 2
        || recommendation.status != gameplay::SupportRecommendationStatus::Actionable
        || recommendation.blocked_action != Some(gameplay::TacticalAction::DefeatEnemy(enemy))
    {
        return Err(format!("recomendacion de apoyo inesperada: {support:?}"));
    }
    let repeated = session
        .world()
        .companion_cooperative_support_assessment(erick.entity)
        .map_err(|error| format!("repetir evaluacion de apoyo: {error:?}"))?;
    if repeated != Some(support.clone()) || session.world().save_state().serialize() != before_queries {
        return Err("consultar apoyo produjo mutaciones o un orden no determinista".to_owned());
    }
    let encoded = session.save_state().serialize();
    let decoded = gameplay::GameplaySaveState::deserialize(&encoded)
        .map_err(|error| format!("serializar evaluacion de apoyo: {error}"))?;
    let mut restored = ProjectEchoSession::new();
    restored
        .restore_state(decoded)
        .map_err(|error| format!("restaurar evaluacion de apoyo: {error}"))?;
    if restored
        .world()
        .companion_cooperative_support_assessment(erick.entity)
        .map_err(|error| format!("evaluar apoyo restaurado: {error:?}"))?
        != Some(support.clone())
    {
        return Err("la evaluacion restaurada no fue equivalente".to_owned());
    }
    run_command(
        &mut session,
        "ejecutar transferencia recomendada",
        GameplayCommand::TransferInventoryItem {
            from: provider_a,
            to: needy,
            item: ItemId(2),
            amount: recommendation.amount,
        },
    )?;
    if session.world().inventory(provider_a).map_or(0, |inventory| inventory.quantity(ItemId(2))) != 3
        || session.world().inventory(needy).map_or(0, |inventory| inventory.quantity(ItemId(2))) != 2
    {
        return Err("la transferencia recomendada no conservo las cantidades".to_owned());
    }
    let resolved = session
        .world()
        .companion_cooperative_support_assessment(erick.entity)
        .map_err(|error| format!("reevaluar apoyo resuelto: {error:?}"))?
        .ok_or_else(|| "la reevaluacion de apoyo desaparecio".to_owned())?;
    if !resolved.recommendations.is_empty()
        || resolved.status != gameplay::SupportRecommendationStatus::Informational
    {
        return Err(format!("el bloqueo de municion no se resolvio: {resolved:?}"));
    }
    run_command(
        &mut session,
        "ejecutar ataque tras recibir apoyo",
        GameplayCommand::AttackEnemy { player: needy, enemy },
    )?;

    let mut insufficient = ProjectEchoSession::new();
    let insufficient_needy = insufficient.ids.first_player;
    let insufficient_provider = insufficient
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear proveedor insuficiente: {error:?}"))?;
    insufficient
        .world_mut()
        .initialize_inventory(insufficient_provider, 20)
        .map_err(|error| format!("inicializar proveedor insuficiente: {error:?}"))?;
    run_command(
        &mut insufficient,
        "crear ERICK sin cantidad suficiente",
        GameplayCommand::CreateCompanion {
            followed_player: insufficient_needy,
            position: Vec3::ZERO,
        },
    )?;
    run_command(
        &mut insufficient,
        "equipar arma sin cantidad suficiente",
        GameplayCommand::EquipPlayerWeapon {
            player: insufficient_needy,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                20,
                20,
                Duration(1),
                WeaponAmmunition {
                    item: ItemId(2),
                    amount_per_attack: 2,
                },
            ),
        },
    )?;
    run_command(
        &mut insufficient,
        "agregar cantidad insuficiente",
        GameplayCommand::AddInventoryItem {
            player: insufficient_provider,
            item: ItemId(2),
            amount: 1,
        },
    )?;
    run_command(
        &mut insufficient,
        "crear amenaza con proveedor insuficiente",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let insufficient_erick = insufficient
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK insuficiente no quedo registrado".to_owned())?;
    let insufficient_assessment = insufficient
        .world()
        .companion_cooperative_support_assessment(insufficient_erick.entity)
        .map_err(|error| format!("evaluar proveedor insuficiente: {error:?}"))?
        .ok_or_else(|| "proveedor insuficiente no fue evaluable".to_owned())?;
    if insufficient_assessment.status != gameplay::SupportRecommendationStatus::Blocked
        || insufficient_assessment.recommendations.first().is_none_or(|recommendation| {
            recommendation.reason != gameplay::SupportRecommendationReason::ProviderInsufficientQuantity
        })
    {
        return Err(format!("no se diagnostico cantidad insuficiente: {insufficient_assessment:?}"));
    }

    let mut capacity = ProjectEchoSession::new();
    let capacity_provider = capacity.ids.first_player;
    let capacity_needy = capacity
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear receptor sin capacidad: {error:?}"))?;
    capacity
        .world_mut()
        .initialize_inventory(capacity_needy, 1)
        .map_err(|error| format!("inicializar receptor sin capacidad: {error:?}"))?;
    run_command(
        &mut capacity,
        "crear ERICK para receptor sin capacidad",
        GameplayCommand::CreateCompanion {
            followed_player: capacity_needy,
            position: Vec3::ZERO,
        },
    )?;
    run_command(
        &mut capacity,
        "equipar arma del receptor sin capacidad",
        GameplayCommand::EquipPlayerWeapon {
            player: capacity_needy,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                20,
                20,
                Duration(1),
                WeaponAmmunition {
                    item: ItemId(2),
                    amount_per_attack: 2,
                },
            ),
        },
    )?;
    run_command(
        &mut capacity,
        "agregar municion al proveedor de capacidad",
        GameplayCommand::AddInventoryItem {
            player: capacity_provider,
            item: ItemId(2),
            amount: 3,
        },
    )?;
    run_command(
        &mut capacity,
        "crear amenaza para receptor sin capacidad",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let capacity_erick = capacity
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK de capacidad no quedo registrado".to_owned())?;
    let capacity_assessment = capacity
        .world()
        .companion_cooperative_support_assessment(capacity_erick.entity)
        .map_err(|error| format!("evaluar receptor sin capacidad: {error:?}"))?
        .ok_or_else(|| "receptor sin capacidad no fue evaluable".to_owned())?;
    if capacity_assessment.status != gameplay::SupportRecommendationStatus::Blocked
        || capacity_assessment.recommendations.first().is_none_or(|recommendation| {
            recommendation.reason != gameplay::SupportRecommendationReason::ReceiverCapacityExceeded
        })
    {
        return Err(format!("no se diagnostico capacidad insuficiente: {capacity_assessment:?}"));
    }

    let mut defeated_provider_session = ProjectEchoSession::new();
    let defeated_needy = defeated_provider_session.ids.first_player;
    let defeated_provider = defeated_provider_session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear proveedor derrotado: {error:?}"))?;
    defeated_provider_session
        .world_mut()
        .initialize_inventory(defeated_provider, 20)
        .map_err(|error| format!("inicializar proveedor derrotado: {error:?}"))?;
    run_command(
        &mut defeated_provider_session,
        "agregar municion al proveedor derrotado",
        GameplayCommand::AddInventoryItem {
            player: defeated_provider,
            item: ItemId(2),
            amount: 3,
        },
    )?;
    run_command(
        &mut defeated_provider_session,
        "crear ERICK para proveedor derrotado",
        GameplayCommand::CreateCompanion {
            followed_player: defeated_needy,
            position: Vec3::ZERO,
        },
    )?;
    run_command(
        &mut defeated_provider_session,
        "equipar arma del jugador con proveedor derrotado",
        GameplayCommand::EquipPlayerWeapon {
            player: defeated_needy,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                20,
                20,
                Duration(1),
                WeaponAmmunition {
                    item: ItemId(2),
                    amount_per_attack: 2,
                },
            ),
        },
    )?;
    let mut defeat_configuration = EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO);
    defeat_configuration.weapon = WeaponConfiguration::new(AttackType::Kinetic, 100, 20, Duration(1));
    run_command(
        &mut defeated_provider_session,
        "crear atacante del proveedor derrotado",
        GameplayCommand::CreateEnemy {
            configuration: defeat_configuration,
        },
    )?;
    let defeat_enemy = defeated_provider_session
        .world()
        .enemies()
        .next()
        .map(|enemy| enemy.entity)
        .ok_or_else(|| "no se creo el atacante del proveedor derrotado".to_owned())?;
    run_command(
        &mut defeated_provider_session,
        "derrotar proveedor mediante combate real",
        GameplayCommand::EnemyAttackPlayer {
            enemy: defeat_enemy,
            player: defeated_provider,
        },
    )?;
    let defeated_erick = defeated_provider_session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK del proveedor derrotado no quedo registrado".to_owned())?;
    let defeated_assessment = defeated_provider_session
        .world()
        .companion_cooperative_support_assessment(defeated_erick.entity)
        .map_err(|error| format!("evaluar proveedor derrotado: {error:?}"))?
        .ok_or_else(|| "proveedor derrotado no fue evaluable".to_owned())?;
    if defeated_assessment.status != gameplay::SupportRecommendationStatus::Blocked
        || defeated_assessment
            .recommendations
            .first()
            .is_none_or(|recommendation| {
                recommendation.reason != gameplay::SupportRecommendationReason::NoProvider
                    || recommendation.provider.is_some()
            })
        || defeated_assessment.active_players.contains(&defeated_provider)
        || defeated_provider_session
            .world()
            .inventory(defeated_provider)
            .map_or(0, |inventory| inventory.quantity(ItemId(2)))
            != 3
    {
        return Err(format!("el proveedor derrotado fue considerado viable: {defeated_assessment:?}"));
    }
    let defeated_before = defeated_provider_session.world().save_state().serialize();
    let defeated_repeat = defeated_provider_session
        .world()
        .companion_cooperative_support_assessment(defeated_erick.entity)
        .map_err(|error| format!("repetir evaluacion del proveedor derrotado: {error:?}"))?;
    if defeated_repeat != Some(defeated_assessment)
        || defeated_provider_session.world().save_state().serialize() != defeated_before
    {
        return Err("la consulta del proveedor derrotado no fue inmutable".to_owned());
    }

    let inactive_result = session.update([GameplayCommand::SetPlayerActive {
        player: provider_a,
        active: false,
    }]);
    if inactive_result.first().is_none_or(|result| {
        !matches!(result.status, CommandStatus::Success { .. })
    }) {
        return Err("no se pudo desactivar el proveedor para la regresion".to_owned());
    }
    run_command(
        &mut session,
        "crear amenaza para excluir proveedor inactivo",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let inactive_assessment = session
        .world()
        .companion_cooperative_support_assessment(erick.entity)
        .map_err(|error| format!("evaluar proveedor inactivo: {error:?}"))?
        .ok_or_else(|| "la evaluacion con proveedor inactivo no estuvo disponible".to_owned())?;
    if inactive_assessment.status != gameplay::SupportRecommendationStatus::Actionable
        || !inactive_assessment.active_players.contains(&needy)
        || inactive_assessment.active_players.contains(&provider_a)
        || inactive_assessment
            .recommendations
            .first()
            .and_then(|recommendation| recommendation.provider)
            != Some(provider_b)
    {
        return Err(format!("el proveedor inactivo fue considerado valido: {inactive_assessment:?}"));
    }

    let invalid_before = session.world().save_state().serialize();
    let invalid_result = session.update([GameplayCommand::TransferInventoryItem {
        from: provider_b,
        to: needy,
        item: ItemId(99_999),
        amount: 1,
    }]);
    if invalid_result.first().is_none_or(|result| {
        !matches!(result.status, CommandStatus::Rejected { .. } | CommandStatus::Invalid { .. })
    }) || session.world().save_state().serialize() != invalid_before
    {
        return Err("la transferencia de objeto indefinido no fue rechazada atomicamente".to_owned());
    }
    println!(
        "APOYO COOPERATIVO DE ERICK COMPLETADO: necesitaba={}, proveedor={}, municion=2, reevaluado=true, insuficiente=true, capacidad=true, restaurada=true",
        needy, provider_a
    );
    Ok(())
}

fn run_erick_cooperative_readiness_scenario() -> Result<(), String> {
    let mut prepared = ProjectEchoSession::new();
    let prepared_player = prepared.ids.first_player;
    run_command(
        &mut prepared,
        "crear ERICK para equipo preparado",
        GameplayCommand::CreateCompanion {
            followed_player: prepared_player,
            position: Vec3::ZERO,
        },
    )?;
    let prepared_erick = prepared
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK preparado no quedo registrado".to_owned())?;
    run_command(
        &mut prepared,
        "equipar arma lista para encuentro",
        GameplayCommand::EquipPlayerWeapon {
            player: prepared_player,
            weapon: WeaponConfiguration::new(AttackType::Kinetic, 20, 20, Duration(1)),
        },
    )?;
    run_command(
        &mut prepared,
        "crear amenaza para equipo preparado",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let prepared_enemy = prepared
        .world()
        .enemies()
        .next()
        .map(|enemy| enemy.entity)
        .ok_or_else(|| "amenaza preparada no quedo registrada".to_owned())?;
    let prepared_before = prepared.world().save_state().serialize();
    let prepared_assessment = prepared
        .world()
        .companion_cooperative_encounter_readiness(prepared_erick.entity)
        .map_err(|error| format!("evaluar equipo preparado: {error:?}"))?
        .ok_or_else(|| "equipo preparado no fue evaluable".to_owned())?;
    let prepared_repeat = prepared
        .world()
        .companion_cooperative_encounter_readiness(prepared_erick.entity)
        .map_err(|error| format!("repetir equipo preparado: {error:?}"))?;
    if prepared_assessment.status != gameplay::CooperativeEncounterReadinessStatus::Prepared
        || prepared_assessment.threats != vec![prepared_enemy]
        || !prepared_assessment.blockers.is_empty()
        || prepared_assessment.support_opportunities.len() != 0
        || prepared_repeat != Some(prepared_assessment.clone())
        || prepared.world().save_state().serialize() != prepared_before
    {
        return Err(format!("evaluacion de equipo preparado inesperada: {prepared_assessment:?}"));
    }

    let mut blocked = ProjectEchoSession::new();
    let blocked_player = blocked.ids.first_player;
    let provider = blocked
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear proveedor de preparacion: {error:?}"))?;
    let inactive = blocked
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador inactivo de preparacion: {error:?}"))?;
    blocked
        .world_mut()
        .initialize_inventory(provider, 20)
        .map_err(|error| format!("inicializar inventario de preparacion: {error:?}"))?;
    blocked
        .world_mut()
        .initialize_inventory(inactive, 20)
        .map_err(|error| format!("inicializar inventario inactivo: {error:?}"))?;
    run_command(
        &mut blocked,
        "crear ERICK para equipo bloqueado",
        GameplayCommand::CreateCompanion {
            followed_player: blocked_player,
            position: Vec3::ZERO,
        },
    )?;
    let blocked_erick = blocked
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK bloqueado no quedo registrado".to_owned())?;
    run_command(
        &mut blocked,
        "equipar arma que requiere municion",
        GameplayCommand::EquipPlayerWeapon {
            player: blocked_player,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                100,
                20,
                Duration(1),
                WeaponAmmunition {
                    item: ItemId(2),
                    amount_per_attack: 2,
                },
            ),
        },
    )?;
    run_command(
        &mut blocked,
        "preparar municion del proveedor",
        GameplayCommand::AddInventoryItem {
            player: provider,
            item: ItemId(2),
            amount: 2,
        },
    )?;
    run_command(
        &mut blocked,
        "crear amenaza de preparacion",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let blocked_enemy = blocked
        .world()
        .enemies()
        .next()
        .map(|enemy| enemy.entity)
        .ok_or_else(|| "amenaza bloqueada no quedo registrada".to_owned())?;
    run_command(
        &mut blocked,
        "excluir jugador inactivo de preparacion",
        GameplayCommand::SetPlayerActive {
            player: inactive,
            active: false,
        },
    )?;
    let blocked_before = blocked.world().save_state().serialize();
    let blocked_assessment = blocked
        .world()
        .companion_cooperative_encounter_readiness(blocked_erick.entity)
        .map_err(|error| format!("evaluar equipo bloqueado: {error:?}"))?
        .ok_or_else(|| "equipo bloqueado no fue evaluable".to_owned())?;
    if blocked_assessment.status != gameplay::CooperativeEncounterReadinessStatus::Blocked
        || blocked_assessment.threats != vec![blocked_enemy]
        || blocked_assessment.blockers.is_empty()
        || blocked_assessment.active_players.contains(&inactive)
        || blocked_assessment.support_opportunities.len() != 1
        || blocked_assessment.support_opportunities[0].provider != Some(provider)
        || !blocked_assessment.actions.iter().any(|action| {
            matches!(
                action,
                gameplay::CooperativePreparationAction::TransferInventory {
                    from,
                    to,
                    item: ItemId(2),
                    amount: 2,
                } if *from == provider && *to == blocked_player
            )
        })
    {
        return Err(format!("evaluacion de equipo bloqueado inesperada: {blocked_assessment:?}"));
    }
    let blocked_repeat = blocked
        .world()
        .companion_cooperative_encounter_readiness(blocked_erick.entity)
        .map_err(|error| format!("repetir equipo bloqueado: {error:?}"))?;
    if blocked_repeat != Some(blocked_assessment.clone())
        || blocked.world().save_state().serialize() != blocked_before
    {
        return Err("la evaluacion bloqueada no fue determinista o mutó el mundo".to_owned());
    }
    let encoded = blocked.save_state().serialize();
    let decoded = gameplay::GameplaySaveState::deserialize(&encoded)
        .map_err(|error| format!("serializar preparacion cooperativa: {error}"))?;
    let mut restored = ProjectEchoSession::new();
    restored
        .restore_state(decoded)
        .map_err(|error| format!("restaurar preparacion cooperativa: {error}"))?;
    if restored
        .world()
        .companion_cooperative_encounter_readiness(blocked_erick.entity)
        .map_err(|error| format!("evaluar preparacion restaurada: {error:?}"))?
        != Some(blocked_assessment.clone())
    {
        return Err("la preparacion restaurada no fue equivalente".to_owned());
    }
    run_command(
        &mut blocked,
        "ejecutar apoyo de preparacion",
        GameplayCommand::TransferInventoryItem {
            from: provider,
            to: blocked_player,
            item: ItemId(2),
            amount: 2,
        },
    )?;
    let after_support = blocked
        .world()
        .companion_cooperative_encounter_readiness(blocked_erick.entity)
        .map_err(|error| format!("reevaluar preparacion tras apoyo: {error:?}"))?
        .ok_or_else(|| "la preparacion desaparecio tras el apoyo".to_owned())?;
    if after_support.status
            != gameplay::CooperativeEncounterReadinessStatus::PreparedWithReservations
        || !after_support.blockers.is_empty()
        || !after_support.support_opportunities.is_empty()
    {
        return Err(format!("el bloqueo no se resolvio tras el apoyo: {after_support:?}"));
    }
    run_command(
        &mut blocked,
        "ejecutar accion tactica preparada",
        GameplayCommand::AttackEnemy {
            player: blocked_player,
            enemy: blocked_enemy,
        },
    )?;
    let after_defeat = blocked
        .world()
        .companion_cooperative_encounter_readiness(blocked_erick.entity)
        .map_err(|error| format!("evaluar amenaza derrotada: {error:?}"))?
        .ok_or_else(|| "la evaluacion tras derrota no estuvo disponible".to_owned())?;
    if after_defeat.status != gameplay::CooperativeEncounterReadinessStatus::NotEvaluable
        || !after_defeat.threats.is_empty()
    {
        return Err(format!("la amenaza derrotada siguio pendiente: {after_defeat:?}"));
    }

    let mut no_threat = ProjectEchoSession::new();
    let no_threat_player = no_threat.ids.first_player;
    run_command(
        &mut no_threat,
        "crear ERICK sin amenaza para preparacion",
        GameplayCommand::CreateCompanion {
            followed_player: no_threat_player,
            position: Vec3::ZERO,
        },
    )?;
    let no_threat_erick = no_threat
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK sin amenaza no quedo registrado".to_owned())?;
    let no_threat_before = no_threat.world().save_state().serialize();
    let no_threat_assessment = no_threat
        .world()
        .companion_cooperative_encounter_readiness(no_threat_erick.entity)
        .map_err(|error| format!("evaluar ausencia de amenaza: {error:?}"))?
        .ok_or_else(|| "ausencia de amenaza no fue evaluable".to_owned())?;
    if no_threat_assessment.status != gameplay::CooperativeEncounterReadinessStatus::NotEvaluable
        || no_threat_assessment.conclusive
        || !no_threat_assessment.threats.is_empty()
        || no_threat.world().save_state().serialize() != no_threat_before
    {
        return Err(format!("se invento preparacion sin amenaza: {no_threat_assessment:?}"));
    }
    println!(
        "PREPARACION COOPERATIVA DE ERICK COMPLETADA: preparado=true, bloqueo=true, apoyo=true, reevaluado=true, amenaza_derrotada=true, sin_evidencia=no_evaluable, restaurada=true"
    );
    Ok(())
}

fn main() {
    if let Err(error) = run_scenario() {
        eprintln!("ESCENARIO FALLIDO: {error}");
        std::process::exit(1);
    }
}

fn run_persistence_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let first = session.ids.first_player;
    let second = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador para persistencia: {error:?}"))?;
    let third = session
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear tercer jugador para persistencia: {error:?}"))?;
    session
        .world_mut()
        .initialize_inventory(second, 20)
        .map_err(|error| format!("inicializar inventario para persistencia: {error:?}"))?;
    session
        .world_mut()
        .initialize_inventory(third, 20)
        .map_err(|error| format!("inicializar inventario del tercer jugador: {error:?}"))?;
    run_command(
        &mut session,
        "agregar munición al primer jugador antes de guardar",
        GameplayCommand::AddInventoryItem {
            player: first,
            item: ItemId(2),
            amount: 3,
        },
    )?;
    run_command(
        &mut session,
        "agregar munición al segundo jugador antes de guardar",
        GameplayCommand::AddInventoryItem {
            player: second,
            item: ItemId(3),
            amount: 2,
        },
    )?;
    let persistence_origin = DestinationId::new(5701);
    let persistence_destination = DestinationId::new(5702);
    for (id, position) in [
        (persistence_origin, Vec3::ZERO),
        (persistence_destination, Vec3::ZERO),
    ] {
        session
            .register_destination(Destination {
                id,
                destination_type: DestinationType::Planet,
                position,
                travel_duration: Duration(4),
                fuel_cost: 2,
                discovery: DiscoveryState::Discovered,
            })
            .map_err(|error| format!("registrar destino de persistencia: {error:?}"))?;
    }
    session
        .world_mut()
        .register_navigation_route(NavigationRoute {
            from: persistence_origin,
            to: persistence_destination,
            distance: 4.0,
            duration: Duration(4),
            fuel_cost: 2,
            state: NavigationRouteState::Available,
        })
        .map_err(|error| format!("registrar ruta de persistencia: {error:?}"))?;
    session
        .world_mut()
        .register_environmental_hazard(EnvironmentalHazard::new(
            HazardId::new(5701),
            gameplay::EnvironmentalHazardType::SpaceStorm,
            Vec3 { x: 2.0, y: 0.0, z: 0.0 },
            5.0,
            7,
            Duration(10),
            true,
        ))
        .map_err(|error| format!("registrar peligro de persistencia: {error:?}"))?;
    session
        .world_mut()
        .register_spatial_encounter(SpatialEncounter::new(
            EncounterId::new(5701),
            SpatialEncounterKind::Signal {
                destination: persistence_destination,
            },
            Vec3 { x: 2.0, y: 0.0, z: 0.0 },
            4.0,
        ))
        .map_err(|error| format!("registrar encuentro de persistencia: {error:?}"))?;
    session
        .world_mut()
        .register_derelict(
            Derelict::new(DerelictId::new(5701), Vec3 { x: 2.0, y: 0.0, z: 0.0 }, 2.0)
                .with_rewards(
                    vec![DerelictReward {
                        item: ItemId(3),
                        amount: 1,
                    }],
                    RewardDistributionPolicy::Shared,
                ),
        )
        .map_err(|error| format!("registrar derelicto de persistencia: {error:?}"))?;
    session
        .world_mut()
        .register_functional_world(FunctionalWorld::new(
            persistence_destination,
            3.0,
            vec![
                WorldActivity::Trade(TradeMarket {
                    offers: vec![TradeOffer {
                        item: ItemId(3),
                        buy_price: 2,
                        sell_price: 1,
                        stock: 4,
                    }],
                }),
                WorldActivity::Resource(ResourceSite {
                    item: ItemId(3),
                    available: 4,
                    extraction_amount: 2,
                }),
            ],
        ))
        .map_err(|error| format!("registrar mundo funcional de persistencia: {error:?}"))?;
    run_command(
        &mut session,
        "extraer stock de recursos para persistencia",
        GameplayCommand::ExtractWorldResource { player: first, destination: persistence_destination },
    )?;
    run_command(
        &mut session,
        "agotar stock de recursos para persistencia",
        GameplayCommand::ExtractWorldResource { player: first, destination: persistence_destination },
    )?;
    run_command(
        &mut session,
        "vender recursos para persistencia",
        GameplayCommand::SellWorldItem { player: first, destination: persistence_destination, item: ItemId(3), amount: 2 },
    )?;
    run_command(
        &mut session,
        "comprar recurso para persistencia",
        GameplayCommand::BuyWorldItem { player: first, destination: persistence_destination, item: ItemId(3), amount: 1 },
    )?;
    let collected_collectible = session
        .world_mut()
        .create_collectible(ItemId(2), 1, Vec3::ZERO, 2.0)
        .map_err(|error| format!("crear coleccionable recogido: {error:?}"))?;
    run_command(
        &mut session,
        "recoger coleccionable antes de guardar",
        GameplayCommand::CollectItem { player: third, collectible: collected_collectible, amount: 1 },
    )?;
    let pending_collectible = session
        .world_mut()
        .create_collectible(ItemId(2), 1, Vec3::ZERO, 2.0)
        .map_err(|error| format!("crear coleccionable pendiente: {error:?}"))?;
    run_command(
        &mut session,
        "crear enemigo vivo para persistencia",
        GameplayCommand::CreateEnemy { configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Charger, Vec3::ZERO) },
    )?;
    let live_enemy = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event { GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy), _ => None })
        .ok_or_else(|| "no se registro el enemigo vivo".to_owned())?;
    run_command(
        &mut session,
        "equipar arma para derrota persistida",
        GameplayCommand::EquipPlayerWeapon { player: first, weapon: WeaponConfiguration::with_ammunition(AttackType::Kinetic, 200, 20, Duration(1), WeaponAmmunition { item: ItemId(2), amount_per_attack: 1 }) },
    )?;
    run_command(
        &mut session,
        "crear enemigo derrotado para persistencia",
        GameplayCommand::CreateEnemy { configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO) },
    )?;
    let defeated_enemy = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event { GameplayEvent::EnemyCreated { enemy, .. } if *enemy != live_enemy => Some(*enemy), _ => None })
        .ok_or_else(|| "no se registro el enemigo derrotado".to_owned())?;
    run_command(
        &mut session,
        "derrotar enemigo para persistencia",
        GameplayCommand::AttackEnemy { player: first, enemy: defeated_enemy },
    )?;
    let defeated_reward = session
        .world()
        .collectibles()
        .find(|value| value.active && value.item == ItemId(3))
        .copied()
        .ok_or_else(|| "no se genero recompensa persistida".to_owned())?;
    run_command(
        &mut session,
        "recoger recompensa antes de guardar",
        GameplayCommand::CollectItem { player: third, collectible: defeated_reward.entity, amount: 1 },
    )?;
    run_command(
        &mut session,
        "activar encuentro persistido",
        GameplayCommand::ActivateSpatialEncounter { encounter: EncounterId::new(5701) },
    )?;
    session
        .world_mut()
        .register_spatial_encounter(SpatialEncounter::new(EncounterId::new(5702), SpatialEncounterKind::Resource { item: ItemId(3), amount: 1 }, Vec3::ZERO, 4.0))
        .map_err(|error| format!("registrar encuentro resuelto: {error:?}"))?;
    run_command(
        &mut session,
        "activar encuentro que sera resuelto",
        GameplayCommand::ActivateSpatialEncounter { encounter: EncounterId::new(5702) },
    )?;
    run_command(
        &mut session,
        "resolver encuentro persistido",
        GameplayCommand::ResolveSpatialEncounter { player: third, encounter: EncounterId::new(5702) },
    )?;
    run_command(
        &mut session,
        "descubrir derelicto en exploracion",
        GameplayCommand::DiscoverDerelict { player: first, derelict: DerelictId::new(5701) },
    )?;
    run_command(
        &mut session,
        "explorar derelicto en exploracion",
        GameplayCommand::ExploreDerelict { player: first, derelict: DerelictId::new(5701) },
    )?;
    session
        .world_mut()
        .register_derelict(Derelict::new(DerelictId::new(5702), Vec3::ZERO, 2.0))
        .map_err(|error| format!("registrar derelicto resuelto: {error:?}"))?;
    run_command(
        &mut session,
        "descubrir derelicto resuelto",
        GameplayCommand::DiscoverDerelict { player: third, derelict: DerelictId::new(5702) },
    )?;
    run_command(
        &mut session,
        "explorar derelicto resuelto",
        GameplayCommand::ExploreDerelict { player: third, derelict: DerelictId::new(5702) },
    )?;
    run_command(
        &mut session,
        "resolver derelicto persistido",
        GameplayCommand::ResolveDerelict { player: third, derelict: DerelictId::new(5702) },
    )?;
    let before = session.save_state();
    let mut direct = ProjectEchoSession::new();
    direct
        .restore_state(before.clone())
        .map_err(|error| format!("restaurar snapshot completo: {error}"))?;
    if direct.world().destinations().count() != session.world().destinations().count()
        || direct.world().spatial_encounters().count() != session.world().spatial_encounters().count()
        || direct.world().derelicts().count() != session.world().derelicts().count()
        || direct.world().functional_worlds().count() != session.world().functional_worlds().count()
    {
        return Err("el snapshot completo no conservó los dominios de exploración".to_owned());
    }
    let encoded = before.serialize();
    let decoded = gameplay::GameplaySaveState::deserialize(&encoded)
        .map_err(|error| format!("deserializar estado válido: {error}"))?;
    if decoded.serialize() != encoded {
        return Err("la serialización no fue determinista en el round-trip".to_owned());
    }
    let invalid = encoded.replacen("ASTRAL_ECHO_SAVE|1", "ASTRAL_ECHO_SAVE|99", 1);
    if gameplay::GameplaySaveState::deserialize(&invalid).is_ok() {
        return Err("se aceptó un esquema de guardado no soportado".to_owned());
    }
    let player_record = encoded
        .lines()
        .find(|line| line.starts_with("player|"))
        .ok_or_else(|| "el estado no incluyó el registro del jugador".to_owned())?;
    let duplicated = format!("{encoded}{player_record}\n");
    if gameplay::GameplaySaveState::deserialize(&duplicated).is_ok() {
        return Err("se aceptó un identificador de jugador duplicado".to_owned());
    }
    let mut restored = ProjectEchoSession::new();
    restored
        .restore_state(decoded)
        .map_err(|error| format!("restaurar estado válido: {error}"))?;
    if restored.world().entities().active_count() != session.world().entities().active_count()
        || restored.world().player(second).is_none()
        || restored.world().destinations().count() != 2
        || restored.world().navigation_routes().count() != 1
        || restored.world().destination(persistence_destination).is_none()
        || restored.world().inventory(first).map(|value| value.quantity(ItemId(2))) != Some(2)
        || restored.world().inventory(second).map(|value| value.quantity(ItemId(3))) != Some(2)
        || restored.world().ship_fuel(restored.ids.first_ship) != session.world().ship_fuel(session.ids.first_ship)
        || restored.world().environmental_hazards().count() != 1
        || restored.world().spatial_encounters().count() != 2
        || restored.world().derelicts().count() != 2
        || restored.world().functional_worlds().count() != 1
        || restored.world().derelict(DerelictId::new(5701)).map(|value| value.rewards.len()) != Some(1)
        || restored.world().players().count() != 3
        || restored.world().combatant(live_enemy).is_none_or(|value| !value.active || value.is_defeated())
        || restored.world().combatant(defeated_enemy).is_none_or(|value| value.active || !value.is_defeated())
        || restored.world().combatant(defeated_enemy).is_some_and(|value| value.archetype != Some(EnemyArchetype::Shooter))
        || restored.world().collectible(collected_collectible).is_none_or(|value| value.active || value.amount != 0)
        || restored.world().collectible(pending_collectible).is_none_or(|value| !value.active || value.amount != 1)
        || restored.world().collectibles().any(|value| value.entity == defeated_reward.entity && value.active)
        || restored.world().spatial_encounter(EncounterId::new(5701)).map(|value| value.state) != Some(SpatialEncounterState::Active)
        || restored.world().spatial_encounter(EncounterId::new(5702)).map(|value| value.state) != Some(SpatialEncounterState::Resolved)
        || restored.world().derelict(DerelictId::new(5701)).map(|value| value.state) != Some(DerelictState::Exploring)
        || restored.world().derelict(DerelictId::new(5702)).map(|value| value.state) != Some(DerelictState::Resolved)
        || restored.world().functional_world(persistence_destination).is_none_or(|value| !value.activities.iter().any(|activity| matches!(activity, WorldActivity::Resource(site) if site.available == 0)))
        || restored.world().world_trade_offer(persistence_destination, ItemId(3)).is_none_or(|offer| offer.stock != 3)
    {
        return Err("la restauración no conservó identidades, inventarios o recursos".to_owned());
    }
    if restored.save_state().serialize() != before.serialize() {
        return Err("la restauración no conservó el estado persistido".to_owned());
    }
    run_command(
        &mut restored,
        "continuar gameplay tras restaurar persistencia",
        GameplayCommand::AddInventoryItem {
            player: first,
            item: ItemId(2),
            amount: 1,
        },
    )?;
    if restored.world().inventory(first).map(|value| value.quantity(ItemId(2))) != Some(3) {
        return Err("la sesión restaurada no pudo continuar el gameplay".to_owned());
    }
    let target_before_invalid = restored.save_state().serialize();
    let invalid_item = encoded.replace("|2:2", "|999:2");
    let invalid_item_state = gameplay::GameplaySaveState::deserialize(&invalid_item)
        .map_err(|error| format!("deserializar caso de item inválido: {error}"))?;
    if restored.restore_state(invalid_item_state).is_ok()
        || restored.save_state().serialize() != target_before_invalid
    {
        return Err("una carga inválida de inventario dejó mutaciones parciales".to_owned());
    }
    let invalid_reference = encoded.replace("|d:5702|", "|d:999999|");
    let invalid_reference_state = gameplay::GameplaySaveState::deserialize(&invalid_reference)
        .map_err(|error| format!("referencia invalidada: {error}"))?;
    if restored.restore_state(invalid_reference_state).is_ok()
        || restored.save_state().serialize() != target_before_invalid
    {
        return Err("una referencia inexistente dejo mutaciones parciales".to_owned());
    }
    let duplicated_reward = format!("{encoded}processed_reward|{}\n", defeated_enemy.value());
    if gameplay::GameplaySaveState::deserialize(&duplicated_reward).is_ok() {
        return Err("se acepto una recompensa procesada duplicada".to_owned());
    }
    println!(
        "PERSISTENCIA COMPLETADA: jugadores={}, entidades_activas={}, estado_determinista=true",
        restored.world().players().count(),
        restored.world().entities().active_count()
    );
    Ok(())
}

fn run_erick_companion_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let before_entities = session.world().entities().active_count();
    let invalid = session.update([GameplayCommand::CreateCompanion {
        followed_player: EntityId::new(u64::MAX),
        position: Vec3::ZERO,
    }]);
    if !matches!(invalid.first().map(|result| result.status), Some(CommandStatus::Rejected { reason: gameplay::CommandRejection::PlayerNotFound }))
        || session.world().companions().next().is_some()
        || session.world().entities().active_count() != before_entities
    {
        return Err("ERICK aceptó una referencia de jugador inexistente o dejó estado parcial".to_owned());
    }

    let created = session.update([GameplayCommand::CreateCompanion {
        followed_player: player,
        position: Vec3 { x: 2.0, y: 0.0, z: 0.0 },
    }]);
    if !matches!(created.first().map(|result| result.status), Some(CommandStatus::Success { changed: true })) {
        return Err("no se pudo crear ERICK mediante el comando de gameplay".to_owned());
    }
    let erick = session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "la entidad de ERICK no quedó registrada".to_owned())?;
    if erick.followed_player != player
        || session.world().players().count() != 1
        || session.world().group_size() != gameplay::GroupSize::Solo
        || session.world().entities().active_count() != before_entities + 1
    {
        return Err("ERICK alteró el límite o el escalado de jugadores locales".to_owned());
    }
    run_command(
        &mut session,
        "activar acompañamiento de ERICK",
        GameplayCommand::SetCompanionBehavior {
            companion: erick.entity,
            behavior: gameplay::CompanionBehavior::Accompany,
        },
    )?;
    let first_position = erick.position;
    run_command(
        &mut session,
        "mover jugador acompañado",
        GameplayCommand::MovePlayer {
            player,
            direction: Vec3 { x: 1.0, y: 0.0, z: 0.0 },
            delta: Duration(1_000),
        },
    )?;
    run_command(
        &mut session,
        "avanzar seguimiento de ERICK",
        GameplayCommand::AdvanceTime { delta: Duration(1_000) },
    )?;
    let followed_position = session.world().companion(erick.entity).map(|value| value.position)
        .ok_or_else(|| "ERICK desapareció durante el seguimiento".to_owned())?;
    let moved_distance = ((followed_position.x - first_position.x).powi(2)
        + (followed_position.y - first_position.y).powi(2)
        + (followed_position.z - first_position.z).powi(2)).sqrt();
    let player_position = session.world().player(player).map(|value| value.position)
        .ok_or_else(|| "no se pudo leer la posición del jugador acompañado".to_owned())?;
    let distance_to_player = ((followed_position.x - player_position.x).powi(2)
        + (followed_position.y - player_position.y).powi(2)
        + (followed_position.z - player_position.z).powi(2)).sqrt();
    if moved_distance <= 0.0 || moved_distance > gameplay::FOLLOW_SPEED + 0.001
        || (distance_to_player - gameplay::FOLLOW_DISTANCE).abs() > 0.001
    {
        return Err(format!("el seguimiento de ERICK excedió la velocidad o distancia configurada: movió={moved_distance}, distancia={distance_to_player}, posición_jugador={player_position:?}, posición_erick={followed_position:?}"));
    }
    let stationary = followed_position;
    run_command(
        &mut session,
        "avanzar sin necesidad de seguimiento",
        GameplayCommand::AdvanceTime { delta: Duration(1_000) },
    )?;
    if session.world().companion(erick.entity).is_none_or(|value| value.position != stationary) {
        return Err("ERICK se movió innecesariamente dentro de su distancia de seguimiento".to_owned());
    }
    run_command(
        &mut session,
        "poner ERICK en espera",
        GameplayCommand::SetCompanionBehavior {
            companion: erick.entity,
            behavior: gameplay::CompanionBehavior::Waiting,
        },
    )?;
    run_command(
        &mut session,
        "mover jugador con ERICK en espera",
        GameplayCommand::MovePlayer {
            player,
            direction: Vec3 { x: 0.0, y: 1.0, z: 0.0 },
            delta: Duration(1_000),
        },
    )?;
    run_command(
        &mut session,
        "verificar espera de ERICK",
        GameplayCommand::AdvanceTime { delta: Duration(1_000) },
    )?;
    if session.world().companion(erick.entity).is_none_or(|value| value.position != stationary) {
        return Err("ERICK continuó moviéndose mientras esperaba".to_owned());
    }
    run_command(
        &mut session,
        "desactivar jugador acompañado",
        GameplayCommand::SetPlayerActive { player, active: false },
    )?;
    run_command(
        &mut session,
        "verificar indisponibilidad de ERICK",
        GameplayCommand::AdvanceTime { delta: Duration(1) },
    )?;
    if session.world().companion(erick.entity).is_none_or(|value| value.behavior != gameplay::CompanionBehavior::Unavailable) {
        return Err("ERICK no adoptó un estado seguro ante un jugador inactivo".to_owned());
    }
    run_command(
        &mut session,
        "reactivar jugador acompañado",
        GameplayCommand::SetPlayerActive { player, active: true },
    )?;
    let recommendation = session
        .world()
        .companion_recommended_behavior(erick.entity)
        .map_err(|error| format!("consultar guía de ERICK: {error:?}"))?;
    if recommendation != session
        .world()
        .companion_recommended_behavior(erick.entity)
        .map_err(|error| format!("consultar guía repetida de ERICK: {error:?}"))?
    {
        return Err("la recomendación de ERICK no fue determinista".to_owned());
    }
    let original_position = stationary;
    let invalid_behavior = session.update([GameplayCommand::SetCompanionBehavior {
        companion: erick.entity,
        behavior: gameplay::CompanionBehavior::Investigate(gameplay::KnowledgeTarget::Destination(
            DestinationId::new(999_999),
        )),
    }]);
    if !matches!(invalid_behavior.first().map(|result| result.status), Some(CommandStatus::Rejected { reason: gameplay::CommandRejection::CompanionKnowledgeUnavailable }))
        || session.world().companion(erick.entity).is_none_or(|value| value.behavior != gameplay::CompanionBehavior::Unavailable)
    {
        return Err("ERICK aceptó una investigación sin conocimiento disponible".to_owned());
    }
    let target = DestinationId::new(900);
    session
        .register_destination(Destination {
            id: target,
            destination_type: DestinationType::SpacePoint,
            position: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Unknown,
        })
        .map_err(|error| format!("registrar destino de ERICK: {error:?}"))?;
    run_command(
        &mut session,
        "registrar conocimiento para ERICK",
        GameplayCommand::RegisterKnowledge {
            target: gameplay::KnowledgeTarget::Destination(target),
        },
    )?;
    let evidence_position = session
        .world()
        .player(player)
        .map(|value| value.position)
        .ok_or_else(|| "no se pudo leer la posición de evidencia de ERICK".to_owned())?;
    let known_item = session
        .world_mut()
        .create_collectible(ItemId(2), 1, evidence_position, 2.0)
        .map_err(|error| format!("crear evidencia para ERICK: {error:?}"))?;
    run_command(
        &mut session,
        "recoger evidencia para ERICK",
        GameplayCommand::CollectItem {
            player,
            collectible: known_item,
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "identificar conocimiento para ERICK",
        GameplayCommand::IdentifyKnowledge {
            target: gameplay::KnowledgeTarget::Item(ItemId(2)),
        },
    )?;
    run_command(
        &mut session,
        "ordenar investigación de ERICK",
        GameplayCommand::SetCompanionBehavior {
            companion: erick.entity,
            behavior: gameplay::CompanionBehavior::Investigate(gameplay::KnowledgeTarget::Item(ItemId(2))),
        },
    )?;
    let first_decision = session
        .world()
        .companion_decision(erick.entity)
        .map_err(|error| format!("consultar decisión de ERICK: {error:?}"))?;
    let second_decision = session
        .world()
        .companion_decision(erick.entity)
        .map_err(|error| format!("consultar decisión repetida de ERICK: {error:?}"))?;
    if first_decision != second_decision
        || first_decision != gameplay::CompanionBehavior::Investigate(gameplay::KnowledgeTarget::Item(ItemId(2)))
        || session.world().companion(erick.entity).is_none_or(|value| value.position != original_position)
    {
        return Err("la decisión de ERICK no fue determinista o modificó su posición".to_owned());
    }
    run_command(
        &mut session,
        "crear enemigo para apoyo de ERICK",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(
                EnemyArchetype::Shooter,
                player_position,
            ),
        },
    )?;
    let enemy = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "no se creó el enemigo para apoyo de ERICK".to_owned())?;
    run_command(
        &mut session,
        "activar apoyo de combate de ERICK",
        GameplayCommand::SetCompanionBehavior {
            companion: erick.entity,
            behavior: gameplay::CompanionBehavior::CombatSupport,
        },
    )?;
    run_command(
        &mut session,
        "detectar combate con ERICK",
        GameplayCommand::AdvanceTime { delta: Duration(1) },
    )?;
    if session.world().companion_decision(erick.entity)
        .map_err(|error| format!("consultar apoyo de combate de ERICK: {error:?}"))?
        != gameplay::CompanionBehavior::CombatSupport
        || session.world().combatant(erick.entity).is_some()
        || session.world().events().iter().any(|event| {
            matches!(event, GameplayEvent::AttackExecuted { target, .. } if *target == erick.entity)
        })
    {
        return Err("ERICK fue tratado como objetivo o no conservó el apoyo de combate válido".to_owned());
    }
    run_command(
        &mut session,
        "finalizar combate de apoyo de ERICK",
        GameplayCommand::SetEnemyActive { enemy, active: false },
    )?;
    run_command(
        &mut session,
        "volver a acompañamiento tras combate",
        GameplayCommand::AdvanceTime { delta: Duration(1) },
    )?;
    if session.world().companion_decision(erick.entity)
        .map_err(|error| format!("consultar retorno de ERICK: {error:?}"))?
        != gameplay::CompanionBehavior::Accompany
    {
        return Err("ERICK no volvió al acompañamiento al terminar el combate".to_owned());
    }
    let encoded = session.save_state().serialize();
    let decoded = gameplay::GameplaySaveState::deserialize(&encoded)
        .map_err(|error| format!("serializar estado de ERICK: {error}"))?;
    let mut restored = ProjectEchoSession::new();
    restored
        .restore_state(decoded)
        .map_err(|error| format!("restaurar estado de ERICK: {error}"))?;
    if restored.save_state().serialize() != encoded
        || restored.world().companion(erick.entity).is_none_or(|value| {
            value.followed_player != player
                || value.behavior != gameplay::CompanionBehavior::Accompany
        })
    {
        return Err("el round-trip textual de ERICK no conservó su relación o comportamiento".to_owned());
    }
    let invalid_text = encoded.replace(
        &format!("companion|{}|{}|", erick.entity.value(), player.value()),
        &format!("companion|{}|{}|", erick.entity.value(), u64::MAX),
    );
    let mut unchanged = ProjectEchoSession::new();
    let before_invalid = unchanged.save_state().serialize();
    let invalid_save = gameplay::GameplaySaveState::deserialize(&invalid_text)
        .map_err(|error| format!("preparar estado inválido de ERICK: {error}"))?;
    if unchanged.restore_state(invalid_save).is_ok()
        || unchanged.save_state().serialize() != before_invalid
    {
        return Err("la carga inválida de ERICK dejó mutaciones parciales".to_owned());
    }
    println!(
        "ERICK COMPLETADO: entidad={}, jugador={}, comportamiento={:?}, jugadores={}, determinista=true",
        erick.entity, player, gameplay::CompanionBehavior::Accompany, session.world().players().count()
    );
    Ok(())
}

fn run_erick_interaction_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let status_console = session
        .create_ship_status_console(Vec3::ZERO, 2.0)
        .map_err(|error| format!("crear consola para asistencia de ERICK: {error:?}"))?;
    let electrical_panel = session
        .create_electrical_panel(
            Vec3::ZERO,
            2.0,
            ElectricalPanelConfiguration::new(vec![ElectricalBreakerConfiguration {
                id: BreakerId::new(1),
                initial_state: BreakerState::Off,
                target_state: BreakerState::On,
            }]),
        )
        .map_err(|error| format!("crear panel para asistencia de ERICK: {error:?}"))?;
    run_command(
        &mut session,
        "crear ERICK para asistencia de interacciÃ³n",
        GameplayCommand::CreateCompanion {
            followed_player: player,
            position: Vec3 { x: 1.0, y: 0.0, z: 0.0 },
        },
    )?;
    let erick = session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK no quedÃ³ disponible para asistencia".to_owned())?;
    run_command(
        &mut session,
        "desbloquear misiÃ³n para asistencia de ERICK",
        GameplayCommand::UnlockMission {
            mission: el_despertar::EL_DESPERTAR,
        },
    )?;
    run_command(
        &mut session,
        "iniciar misiÃ³n para asistencia de ERICK",
        GameplayCommand::StartMission {
            mission: el_despertar::EL_DESPERTAR,
        },
    )?;
    let recommendation = session
        .world()
        .companion_action_recommendation(erick.entity)
        .map_err(|error| format!("consultar interacciÃ³n recomendada por ERICK: {error:?}"))?
        .ok_or_else(|| "ERICK no identificÃ³ la interacciÃ³n disponible".to_owned())?;
    if recommendation.companion != erick.entity
        || recommendation.player != player
        || recommendation.mode != gameplay::CompanionActionMode::PlayerRequired
        || recommendation.action != gameplay::GuidanceAction::Interact(status_console)
    {
        return Err(format!(
            "ERICK recomendÃ³ una acciÃ³n inesperada: {recommendation:?}"
        ));
    }
    if session
        .world()
        .companion_action_recommendation(erick.entity)
        .map_err(|error| format!("consultar recomendaciÃ³n determinista de ERICK: {error:?}"))?
        != Some(recommendation)
    {
        return Err("la recomendaciÃ³n de ERICK no fue determinista".to_owned());
    }
    let erick_attempt = session.update([GameplayCommand::Interact {
        player: erick.entity,
        target: status_console,
    }]);
    if !matches!(
        erick_attempt.first().map(|result| result.status),
        Some(CommandStatus::Rejected {
            reason: gameplay::CommandRejection::PlayerNotFound
        })
    ) || !matches!(
        session.world().interactable(status_console).map(|value| value.behavior),
        Some(gameplay::InteractionBehavior::Console {
            state: gameplay::ConsoleState::Off,
            ..
        })
    ) {
        return Err("ERICK ejecutÃ³ una interacciÃ³n que exige un jugador o dejÃ³ un cambio parcial".to_owned());
    }
    run_command(
        &mut session,
        "ejecutar por el jugador la interacciÃ³n recomendada por ERICK",
        GameplayCommand::Interact {
            player,
            target: status_console,
        },
    )?;
    if !matches!(
        session.world().interactable(status_console).map(|value| value.behavior),
        Some(gameplay::InteractionBehavior::Console {
            state: gameplay::ConsoleState::On,
            ..
        })
    ) || !session.world().events().iter().any(|event| {
        matches!(event, GameplayEvent::InteractionSucceeded { player: actor, target } if *actor == gameplay::types::PlayerId(player) && *target == status_console)
    }) {
        return Err("la interacciÃ³n recomendada no produjo el efecto real esperado".to_owned());
    }
    let next_recommendation = session
        .world()
        .companion_action_recommendation(erick.entity)
        .map_err(|error| format!("consultar siguiente interacciÃ³n de ERICK: {error:?}"))?
        .ok_or_else(|| "ERICK no actualizÃ³ su recomendaciÃ³n tras el cambio de estado".to_owned())?;
    if next_recommendation.action != gameplay::GuidanceAction::Interact(electrical_panel)
        || next_recommendation == recommendation
    {
        return Err(format!(
            "la recomendaciÃ³n no reflejÃ³ el siguiente estado autoritativo: {next_recommendation:?}"
        ));
    }
    let before_invalid = session.world().save_state().serialize();
    let invalid_target = session.update([GameplayCommand::Interact {
        player,
        target: EntityId::new(u64::MAX),
    }]);
    if !matches!(
        invalid_target.first().map(|result| result.status),
        Some(CommandStatus::Rejected {
            reason: gameplay::CommandRejection::TargetNotFound
        })
    ) || session.world().save_state().serialize() != before_invalid {
        return Err("una interacciÃ³n invÃ¡lida dejÃ³ mutaciones parciales".to_owned());
    }
    run_command(
        &mut session,
        "desactivar jugador para comprobar ERICK indisponible",
        GameplayCommand::SetPlayerActive {
            player,
            active: false,
        },
    )?;
    if session
        .world()
        .companion_action_recommendation(erick.entity)
        .map_err(|error| format!("consultar ERICK indisponible: {error:?}"))?
        .is_some()
        || session.world().companion_decision(erick.entity)
            .map_err(|error| format!("consultar estado de ERICK indisponible: {error:?}"))?
            != gameplay::CompanionBehavior::Unavailable
    {
        return Err("ERICK continuÃ³ ofreciendo acciones con el jugador indisponible".to_owned());
    }
    println!(
        "ASISTENCIA DE ERICK COMPLETADA: recomendaciÃ³n={recommendation:?}, siguiente={next_recommendation:?}, ejecuciÃ³n=jugador"
    );
    Ok(())
}

fn run_erick_tactical_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    run_command(
        &mut session,
        "crear ERICK para anÃ¡lisis tÃ¡ctico",
        GameplayCommand::CreateCompanion {
            followed_player: player,
            position: Vec3::ZERO,
        },
    )?;
    let erick = session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK no quedÃ³ registrado para anÃ¡lisis tÃ¡ctico".to_owned())?;
    run_command(
        &mut session,
        "crear amenaza tÃ¡ctica real",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let enemy = session
        .world()
        .events()
        .iter()
        .find_map(|event| match event {
            GameplayEvent::EnemyCreated { enemy, .. } => Some(*enemy),
            _ => None,
        })
        .ok_or_else(|| "la amenaza tÃ¡ctica no produjo una entidad real".to_owned())?;
    let hazard = gameplay::HazardId::new(9_061);
    run_command(
        &mut session,
        "registrar peligro tÃ¡ctico real",
        GameplayCommand::RegisterEnvironmentalHazard {
            hazard: EnvironmentalHazard::new(
                hazard,
                gameplay::EnvironmentalHazardType::SpaceStorm,
                Vec3::ZERO,
                2.0,
                1,
                Duration(10_000),
                true,
            ),
        },
    )?;
    let state_before_queries = session.world().save_state().serialize();
    let first = session
        .world()
        .companion_tactical_recommendation(erick.entity)
        .map_err(|error| format!("consultar amenaza tÃ¡ctica de ERICK: {error:?}"))?
        .ok_or_else(|| "ERICK no detectÃ³ la amenaza activa".to_owned())?;
    if first.companion != erick.entity
        || first.player != player
        || first.priority != gameplay::TacticalPriority::Critical
        || first.action != gameplay::TacticalAction::DefeatEnemy(enemy)
    {
        return Err(format!("la prioridad tÃ¡ctica inicial fue incorrecta: {first:?}"));
    }
    let repeated = session
        .world()
        .companion_tactical_recommendation(erick.entity)
        .map_err(|error| format!("consultar prioridad tÃ¡ctica repetida: {error:?}"))?;
    if repeated != Some(first) || session.world().save_state().serialize() != state_before_queries {
        return Err("consultar tÃ¡ctica produjo una decisiÃ³n no determinista o mutaciones".to_owned());
    }
    if session.world().combatant(erick.entity).is_some() {
        return Err("ERICK fue registrado como combatiente".to_owned());
    }
    run_command(
        &mut session,
        "equipar arma del jugador para resolver la amenaza",
        GameplayCommand::EquipPlayerWeapon {
            player,
            weapon: WeaponConfiguration::new(AttackType::Kinetic, 100, 20, Duration(1)),
        },
    )?;
    run_command(
        &mut session,
        "derrotar amenaza tÃ¡ctica mediante combate real",
        GameplayCommand::AttackEnemy { player, enemy },
    )?;
    if session
        .world()
        .combatant(enemy)
        .is_none_or(|combatant| combatant.active || !combatant.is_defeated())
    {
        return Err("la amenaza tÃ¡ctica no quedÃ³ derrotada".to_owned());
    }
    let after_defeat = session
        .world()
        .companion_tactical_recommendation(erick.entity)
        .map_err(|error| format!("consultar tÃ¡ctica tras derrota: {error:?}"))?
        .ok_or_else(|| "ERICK no actualizÃ³ la prioridad tras derrotar la amenaza".to_owned())?;
    if after_defeat.priority != gameplay::TacticalPriority::Critical
        || after_defeat.action != gameplay::TacticalAction::WithdrawFromHazard(hazard)
    {
        return Err(format!("la prioridad tras la derrota fue incorrecta: {after_defeat:?}"));
    }
    if session
        .world()
        .companion_tactical_recommendation(erick.entity)
        .map_err(|error| format!("consultar peligro tÃ¡ctico repetido: {error:?}"))?
        != Some(after_defeat)
    {
        return Err("la recomendaciÃ³n del peligro se duplicÃ³ o cambiÃ³ entre consultas".to_owned());
    }
    run_command(
        &mut session,
        "desactivar jugador para bloquear asistencia tÃ¡ctica",
        GameplayCommand::SetPlayerActive {
            player,
            active: false,
        },
    )?;
    if session
        .world()
        .companion_tactical_recommendation(erick.entity)
        .map_err(|error| format!("consultar ERICK sin jugador activo: {error:?}"))?
        .is_some()
    {
        return Err("ERICK ofreciÃ³ asistencia accionable a un jugador inactivo".to_owned());
    }
    let encoded = session.save_state().serialize();
    let decoded = gameplay::GameplaySaveState::deserialize(&encoded)
        .map_err(|error| format!("serializar tÃ¡ctica: {error}"))?;
    let mut restored = ProjectEchoSession::new();
    restored
        .restore_state(decoded)
        .map_err(|error| format!("restaurar tÃ¡ctica: {error}"))?;
    if restored
        .world()
        .companion_tactical_recommendation(erick.entity)
        .map_err(|error| format!("recalcular tÃ¡ctica restaurada: {error:?}"))?
        .is_some()
    {
        return Err("la sesiÃ³n restaurada ofreciÃ³ asistencia con el jugador inactivo".to_owned());
    }
    println!(
        "TACTICA DE ERICK COMPLETADA: amenaza={:?}, prioridad_inicial={:?}, prioridad_posterior={:?}, restaurada_sin_accion=true",
        enemy, first.priority, after_defeat.priority
    );
    Ok(())
}

fn run_erick_cooperative_tactical_scenario() -> Result<(), String> {
    let mut solo = ProjectEchoSession::new();
    let solo_player = solo.ids.first_player;
    run_command(
        &mut solo,
        "crear ERICK para tactica individual",
        GameplayCommand::CreateCompanion {
            followed_player: solo_player,
            position: Vec3::ZERO,
        },
    )?;
    let solo_erick = solo
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK individual no quedo registrado".to_owned())?;
    run_command(
        &mut solo,
        "equipar jugador individual",
        GameplayCommand::EquipPlayerWeapon {
            player: solo_player,
            weapon: WeaponConfiguration::new(AttackType::Kinetic, 100, 20, Duration(1)),
        },
    )?;
    run_command(
        &mut solo,
        "crear amenaza individual",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let solo_enemy = solo
        .world()
        .enemies()
        .next()
        .map(|enemy| enemy.entity)
        .ok_or_else(|| "amenaza individual no registrada".to_owned())?;
    let solo_assessment = solo
        .world()
        .companion_cooperative_tactical_assessment(solo_erick.entity)
        .map_err(|error| format!("evaluar tactica individual: {error:?}"))?
        .ok_or_else(|| "tactica individual no disponible".to_owned())?;
    let solo_recommendation = solo_assessment
        .recommendations
        .iter()
        .find(|recommendation| recommendation.player == solo_player)
        .ok_or_else(|| "no hubo recomendacion individual".to_owned())?;
    if solo_assessment.active_players != vec![solo_player]
        || solo_assessment.threats != vec![solo_enemy]
        || solo_recommendation.status != gameplay::TacticalAssignmentStatus::Actionable
        || !solo_recommendation.preferred
        || solo_recommendation.action != Some(gameplay::TacticalAction::DefeatEnemy(solo_enemy))
    {
        return Err(format!("tactica individual inesperada: {solo_assessment:?}"));
    }
    run_command(
        &mut solo,
        "ejecutar recomendacion tactica individual",
        GameplayCommand::AttackEnemy {
            player: solo_player,
            enemy: solo_enemy,
        },
    )?;
    let solo_after = solo
        .world()
        .companion_cooperative_tactical_assessment(solo_erick.entity)
        .map_err(|error| format!("recalcular tactica tras derrota: {error:?}"))?
        .ok_or_else(|| "tactica individual desaparecio tras la derrota".to_owned())?;
    if !solo_after.threats.is_empty() || solo_after.conclusive {
        return Err(format!("enemigo derrotado siguio siendo objetivo: {solo_after:?}"));
    }

    let mut duo = ProjectEchoSession::new();
    let duo_a = duo.ids.first_player;
    let duo_b = duo
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador tactico B: {error:?}"))?;
    run_command(
        &mut duo,
        "crear ERICK para tactica de duo",
        GameplayCommand::CreateCompanion {
            followed_player: duo_a,
            position: Vec3::ZERO,
        },
    )?;
    let duo_erick = duo
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK de duo no quedo registrado".to_owned())?;
    run_command(
        &mut duo,
        "equipar jugador tactico A",
        GameplayCommand::EquipPlayerWeapon {
            player: duo_a,
            weapon: WeaponConfiguration::new(AttackType::Kinetic, 20, 20, Duration(1)),
        },
    )?;
    run_command(
        &mut duo,
        "crear amenaza de duo",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Charger, Vec3::ZERO),
        },
    )?;
    let duo_enemy = duo
        .world()
        .enemies()
        .next()
        .map(|enemy| enemy.entity)
        .ok_or_else(|| "amenaza de duo no registrada".to_owned())?;
    let duo_assessment = duo
        .world()
        .companion_cooperative_tactical_assessment(duo_erick.entity)
        .map_err(|error| format!("evaluar tactica de duo: {error:?}"))?
        .ok_or_else(|| "tactica de duo no disponible".to_owned())?;
    let duo_preferred = duo_assessment
        .recommendations
        .iter()
        .filter(|recommendation| recommendation.preferred)
        .collect::<Vec<_>>();
    if duo_assessment.active_players != vec![duo_a, duo_b]
        || duo_assessment.threats != vec![duo_enemy]
        || duo_preferred.len() != 1
        || duo_preferred[0].player != duo_a
        || duo_preferred[0].threat != Some(duo_enemy)
    {
        return Err(format!("tactica de duo duplicada o inestable: {duo_assessment:?}"));
    }

    let mut trio = ProjectEchoSession::new();
    let trio_a = trio.ids.first_player;
    let trio_b = trio
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador tactico trio B: {error:?}"))?;
    let trio_c = trio
        .world_mut()
        .create_player()
        .map_err(|error| format!("crear jugador tactico trio C: {error:?}"))?;
    run_command(
        &mut trio,
        "crear ERICK para tactica de trio",
        GameplayCommand::CreateCompanion {
            followed_player: trio_a,
            position: Vec3::ZERO,
        },
    )?;
    let trio_erick = trio
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK de trio no quedo registrado".to_owned())?;
    for player in [trio_a, trio_b] {
        run_command(
            &mut trio,
            "equipar jugador tactico de trio",
            GameplayCommand::EquipPlayerWeapon {
                player,
                weapon: WeaponConfiguration::new(AttackType::Kinetic, 20, 20, Duration(1)),
            },
        )?;
    }
    for _ in 0..2 {
        run_command(
            &mut trio,
            "crear amenaza de trio",
            GameplayCommand::CreateEnemy {
                configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Charger, Vec3::ZERO),
            },
        )?;
    }
    let trio_threats = trio
        .world()
        .enemies()
        .map(|enemy| enemy.entity)
        .collect::<Vec<_>>();
    if trio_threats.len() != 2 {
        return Err(format!("no se registraron las dos amenazas de trio: {trio_threats:?}"));
    }
    let trio_before = trio.world().save_state().serialize();
    let trio_assessment = trio
        .world()
        .companion_cooperative_tactical_assessment(trio_erick.entity)
        .map_err(|error| format!("evaluar tactica de trio: {error:?}"))?
        .ok_or_else(|| "tactica de trio no disponible".to_owned())?;
    let trio_repeat = trio
        .world()
        .companion_cooperative_tactical_assessment(trio_erick.entity)
        .map_err(|error| format!("repetir tactica de trio: {error:?}"))?;
    let preferred_targets = trio_assessment
        .recommendations
        .iter()
        .filter(|recommendation| recommendation.preferred)
        .map(|recommendation| (recommendation.player, recommendation.threat))
        .collect::<Vec<_>>();
    if trio_assessment.active_players != vec![trio_a, trio_b, trio_c]
        || trio_assessment.threats != trio_threats
        || preferred_targets.len() != 2
        || preferred_targets[0].0 == preferred_targets[1].0
        || preferred_targets[0].1 == preferred_targets[1].1
        || trio_repeat != Some(trio_assessment.clone())
        || trio.world().save_state().serialize() != trio_before
    {
        return Err(format!("tactica de trio no fue determinista: {trio_assessment:?}"));
    }
    let encoded = trio.world().save_state().serialize();
    let decoded = gameplay::GameplaySaveState::deserialize(&encoded)
        .map_err(|error| format!("serializar tactica cooperativa: {error}"))?;
    let mut restored = ProjectEchoSession::new();
    restored
        .restore_state(decoded)
        .map_err(|error| format!("restaurar tactica cooperativa: {error}"))?;
    if restored
        .world()
        .companion_cooperative_tactical_assessment(trio_erick.entity)
        .map_err(|error| format!("consultar tactica restaurada: {error:?}"))?
        != Some(trio_assessment.clone())
    {
        return Err("la tactica cooperativa restaurada no fue equivalente".to_owned());
    }

    let mut ammunition_missing = ProjectEchoSession::new();
    let ammunition_player = ammunition_missing.ids.first_player;
    run_command(
        &mut ammunition_missing,
        "crear ERICK para tactica sin municion",
        GameplayCommand::CreateCompanion {
            followed_player: ammunition_player,
            position: Vec3::ZERO,
        },
    )?;
    let ammunition_erick = ammunition_missing
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK sin municion no quedo registrado".to_owned())?;
    run_command(
        &mut ammunition_missing,
        "equipar arma que requiere municion",
        GameplayCommand::EquipPlayerWeapon {
            player: ammunition_player,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                20,
                20,
                Duration(1),
                WeaponAmmunition {
                    item: ItemId(2),
                    amount_per_attack: 1,
                },
            ),
        },
    )?;
    run_command(
        &mut ammunition_missing,
        "crear amenaza sin municion",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Charger, Vec3::ZERO),
        },
    )?;
    let ammunition_enemy = ammunition_missing
        .world()
        .enemies()
        .next()
        .map(|enemy| enemy.entity)
        .ok_or_else(|| "amenaza sin municion no registrada".to_owned())?;
    let ammunition_assessment = ammunition_missing
        .world()
        .companion_cooperative_tactical_assessment(ammunition_erick.entity)
        .map_err(|error| format!("evaluar falta de municion: {error:?}"))?
        .ok_or_else(|| "falta de municion no evaluable".to_owned())?;
    if ammunition_assessment.recommendations.len() != 1
        || ammunition_assessment.recommendations[0].status
            != gameplay::TacticalAssignmentStatus::Blocked
        || ammunition_assessment.recommendations[0].reason
            != gameplay::TacticalAssignmentReason::MissingAmmunition
        || ammunition_assessment.unassigned_threats != vec![ammunition_enemy]
        || ammunition_assessment.conclusive
    {
        return Err(format!("falta de municion no fue diagnosticada: {ammunition_assessment:?}"));
    }

    run_command(
        &mut ammunition_missing,
        "desactivar jugador para bloquear tactica cooperativa",
        GameplayCommand::SetPlayerActive {
            player: ammunition_player,
            active: false,
        },
    )?;
    if ammunition_missing
        .world()
        .companion_cooperative_tactical_assessment(ammunition_erick.entity)
        .map_err(|error| format!("consultar jugador tactico inactivo: {error:?}"))?
        .is_some()
    {
        return Err("ERICK ofrecio tactica a un jugador inactivo".to_owned());
    }
    let mut no_threat = ProjectEchoSession::new();
    let no_threat_player = no_threat.ids.first_player;
    run_command(
        &mut no_threat,
        "crear ERICK sin amenazas",
        GameplayCommand::CreateCompanion {
            followed_player: no_threat_player,
            position: Vec3::ZERO,
        },
    )?;
    let no_threat_erick = no_threat
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK sin amenazas no quedo registrado".to_owned())?;
    let no_threat_assessment = no_threat
        .world()
        .companion_cooperative_tactical_assessment(no_threat_erick.entity)
        .map_err(|error| format!("evaluar falta de informacion tactica: {error:?}"))?
        .ok_or_else(|| "la falta de informacion no fue evaluable".to_owned())?;
    if no_threat_assessment.conclusive
        || !no_threat_assessment.threats.is_empty()
        || no_threat_assessment.recommendations.iter().any(|recommendation| {
            recommendation.status != gameplay::TacticalAssignmentStatus::NotEvaluable
                || recommendation.action.is_some()
        })
    {
        return Err(format!("ERICK invento una asignacion sin amenazas: {no_threat_assessment:?}"));
    }
    println!(
        "TACTICA COOPERATIVA DE ERICK COMPLETADA: solo=1 amenaza, duo=1 amenaza/1 asignacion, trio=2 amenazas/2 asignaciones, sin_municion= bloqueado, sin_informacion=no_evaluable, restaurada=true"
    );
    Ok(())
}

fn run_erick_navigation_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let station = session
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estaciÃ³n para navegaciÃ³n de ERICK: {error:?}"))?;
    run_command(
        &mut session,
        "crear ERICK para navegaciÃ³n",
        GameplayCommand::CreateCompanion {
            followed_player: player,
            position: Vec3::ZERO,
        },
    )?;
    let erick = session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK no quedÃ³ registrado para navegaciÃ³n".to_owned())?;
    let origin = DestinationId::new(6_201);
    let destination = DestinationId::new(6_202);
    for value in [
        Destination {
            id: origin,
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        },
        Destination {
            id: destination,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
            travel_duration: Duration(5),
            fuel_cost: 2,
            discovery: DiscoveryState::Discovered,
        },
    ] {
        run_command(
            &mut session,
            "registrar destino conocido para ERICK",
            GameplayCommand::RegisterDestination { destination: value },
        )?;
    }
    let route = NavigationRoute {
        from: origin,
        to: destination,
        distance: 10.0,
        duration: Duration(5),
        fuel_cost: 2,
        state: NavigationRouteState::Available,
    };
    let reverse_route = NavigationRoute {
        from: destination,
        to: origin,
        distance: 10.0,
        duration: Duration(5),
        fuel_cost: 2,
        state: NavigationRouteState::Available,
    };
    run_command(
        &mut session,
        "registrar ruta de ERICK",
        GameplayCommand::RegisterNavigationRoute { route },
    )?;
    run_command(
        &mut session,
        "registrar ruta de regreso de ERICK",
        GameplayCommand::RegisterNavigationRoute {
            route: reverse_route,
        },
    )?;
    let initial = session
        .world()
        .companion_navigation_assessment(erick.entity)
        .map_err(|error| format!("consultar navegaciÃ³n inicial de ERICK: {error:?}"))?
        .ok_or_else(|| "ERICK no produjo contexto de navegaciÃ³n inicial".to_owned())?;
    if !matches!(initial.autopilot, AutopilotState::Inactive)
        || initial.destination.is_some()
        || initial.recommendation.is_some()
    {
        return Err(format!("contexto inicial de navegaciÃ³n inesperado: {initial:?}"));
    }
    run_command(
        &mut session,
        "ocupar estaciÃ³n para ERICK",
        GameplayCommand::Interact {
            player,
            target: station,
        },
    )?;
    run_command(
        &mut session,
        "planificar autopilot observado por ERICK",
        GameplayCommand::PlanAutopilot {
            player,
            pilot_station: station,
            origin,
            destination,
        },
    )?;
    let planned = session
        .world()
        .companion_navigation_assessment(erick.entity)
        .map_err(|error| format!("consultar autopilot planificado de ERICK: {error:?}"))?
        .ok_or_else(|| "ERICK perdiÃ³ el jugador activo al planificar".to_owned())?;
    if !matches!(planned.autopilot, AutopilotState::Planned { .. })
        || planned.destination != Some(destination)
        || planned.route.as_slice() != [origin, destination]
        || planned.eta != Some(Duration(5))
        || planned.fuel_required != Some(2)
        || planned.recommendation
            != Some(gameplay::NavigationAdvice::StartPlannedAutopilot { destination })
    {
        return Err(format!("contexto de autopilot planificado inesperado: {planned:?}"));
    }
    let query_state = session.world().save_state().serialize();
    if session
        .world()
        .companion_navigation_assessment(erick.entity)
        .map_err(|error| format!("repetir consulta de navegaciÃ³n: {error:?}"))?
        != Some(planned.clone())
        || session.world().save_state().serialize() != query_state
    {
        return Err("la consulta de navegaciÃ³n no fue determinista o mutÃ³ el estado".to_owned());
    }
    run_command(
        &mut session,
        "iniciar autopilot observado por ERICK",
        GameplayCommand::StartAutopilot {
            player,
            pilot_station: station,
        },
    )?;
    let encounter = EncounterId::new(6_201);
    run_command(
        &mut session,
        "registrar encuentro conocido en la ruta",
        GameplayCommand::RegisterSpatialEncounter {
            encounter: SpatialEncounter::new(
                encounter,
                SpatialEncounterKind::Resource {
                    item: ItemId(1),
                    amount: 1,
                },
                Vec3::ZERO,
                2.0,
            ),
        },
    )?;
    let transit = session
        .world()
        .companion_navigation_assessment(erick.entity)
        .map_err(|error| format!("consultar autopilot en trÃ¡nsito: {error:?}"))?
        .ok_or_else(|| "ERICK no produjo contexto durante el trÃ¡nsito".to_owned())?;
    if !matches!(transit.autopilot, AutopilotState::InTransit { .. })
        || transit.eta != Some(Duration(5))
        || transit.risks != vec![gameplay::NavigationRisk::KnownEncounter(encounter)]
        || transit.recommendation != Some(gameplay::NavigationAdvice::AddressEncounter(encounter))
    {
        return Err(format!("riesgo de ruta no observado correctamente: {transit:?}"));
    }
    run_command(
        &mut session,
        "activar encuentro observado por ERICK",
        GameplayCommand::ActivateSpatialEncounter { encounter },
    )?;
    run_command(
        &mut session,
        "resolver encuentro observado por ERICK",
        GameplayCommand::ResolveSpatialEncounter { player, encounter },
    )?;
    let after_resolution = session
        .world()
        .companion_navigation_assessment(erick.entity)
        .map_err(|error| format!("consultar ruta tras resolver encuentro: {error:?}"))?
        .ok_or_else(|| "ERICK no produjo contexto tras resolver el encuentro".to_owned())?;
    if after_resolution.risks.iter().any(|risk| {
        *risk == gameplay::NavigationRisk::KnownEncounter(encounter)
    })
        || after_resolution.recommendation
            != Some(gameplay::NavigationAdvice::MonitorAutopilot { destination })
    {
        return Err("la advertencia de encuentro no se retirÃ³ tras una resoluciÃ³n real".to_owned());
    }
    run_command(
        &mut session,
        "completar autopilot observado por ERICK",
        GameplayCommand::AdvanceTime {
            delta: Duration(5),
        },
    )?;
    let completed = session
        .world()
        .companion_navigation_assessment(erick.entity)
        .map_err(|error| format!("consultar autopilot completado de ERICK: {error:?}"))?
        .ok_or_else(|| "ERICK no produjo contexto al completar el viaje".to_owned())?;
    if !matches!(completed.autopilot, AutopilotState::Completed { destination: value } if value == destination)
        || completed.eta.is_some()
        || completed.recommendation.is_some()
    {
        return Err(format!("estado completado de navegaciÃ³n inesperado: {completed:?}"));
    }
    let ship = session.ids.first_ship;
    let fuel = session
        .world()
        .ship_fuel(ship)
        .ok_or_else(|| "faltÃ³ combustible al validar autopilot de ERICK".to_owned())?
        .current();
    run_command(
        &mut session,
        "retirar combustible para evaluar riesgo",
        GameplayCommand::RemoveShipFuel {
            ship,
            amount: fuel,
        },
    )?;
    run_command(
        &mut session,
        "planificar regreso con combustible agotado",
        GameplayCommand::PlanAutopilot {
            player,
            pilot_station: station,
            origin: destination,
            destination: origin,
        },
    )?;
    let insufficient = session
        .world()
        .companion_navigation_assessment(erick.entity)
        .map_err(|error| format!("consultar combustible insuficiente de ERICK: {error:?}"))?
        .ok_or_else(|| "ERICK no produjo advertencia de combustible".to_owned())?;
    if insufficient.fuel_available != Some(0)
        || insufficient.fuel_required != Some(2)
        || insufficient.recommendation
            != Some(gameplay::NavigationAdvice::ReplanAutopilot { destination: origin })
    {
        return Err(format!("riesgo de combustible no informado correctamente: {insufficient:?}"));
    }
    let rejected = session
        .update([GameplayCommand::StartAutopilot {
            player,
            pilot_station: station,
        }])
        .into_iter()
        .next()
        .ok_or_else(|| "inicio con combustible insuficiente sin resultado".to_owned())?;
    if rejected.status
        != (CommandStatus::Rejected {
            reason: gameplay::CommandRejection::InsufficientFuel,
        })
    {
        return Err(format!("autopilot no rechazÃ³ combustible insuficiente: {:?}", rejected.status));
    }
    println!(
        "NAVEGACION DE ERICK COMPLETADA: destino={:?}, eta_inicial={:?}, combustible_disponible_final=0, recomendacion_restaurada=ReplanAutopilot",
        destination, planned.eta
    );
    Ok(())
}

fn run_erick_route_risk_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let station = session
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estaciÃ³n para riesgos de ruta: {error:?}"))?;
    run_command(
        &mut session,
        "crear ERICK para riesgos de ruta",
        GameplayCommand::CreateCompanion {
            followed_player: player,
            position: Vec3::ZERO,
        },
    )?;
    let erick = session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK no quedÃ³ registrado para riesgos de ruta".to_owned())?;
    let origin = DestinationId::new(6_301);
    let destination = DestinationId::new(6_302);
    for value in [
        Destination {
            id: origin,
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        },
        Destination {
            id: destination,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
            travel_duration: Duration(5),
            fuel_cost: 2,
            discovery: DiscoveryState::Discovered,
        },
    ] {
        run_command(
            &mut session,
            "registrar destino para evaluar segmentos",
            GameplayCommand::RegisterDestination { destination: value },
        )?;
    }
    run_command(
        &mut session,
        "registrar segmento para evaluar riesgos",
        GameplayCommand::RegisterNavigationRoute {
            route: NavigationRoute {
                from: origin,
                to: destination,
                distance: 10.0,
                duration: Duration(5),
                fuel_cost: 2,
                state: NavigationRouteState::Available,
            },
        },
    )?;
    let hazards = [
        EnvironmentalHazard::new(
            HazardId::new(6_301),
            gameplay::EnvironmentalHazardType::SpaceStorm,
            Vec3 { x: 5.0, y: 1.0, z: 0.0 },
            1.0,
            1,
            Duration(10_000),
            true,
        ),
        EnvironmentalHazard::new(
            HazardId::new(6_302),
            gameplay::EnvironmentalHazardType::DebrisField,
            Vec3 { x: 5.0, y: 4.0, z: 0.0 },
            1.0,
            1,
            Duration(10_000),
            true,
        ),
        EnvironmentalHazard::new(
            HazardId::new(6_303),
            gameplay::EnvironmentalHazardType::GravitationalAnomaly,
            Vec3 { x: -3.0, y: 0.0, z: 0.0 },
            1.0,
            1,
            Duration(10_000),
            true,
        ),
        EnvironmentalHazard::new(
            HazardId::new(6_304),
            gameplay::EnvironmentalHazardType::SpaceStorm,
            Vec3 { x: 5.0, y: 0.0, z: 0.0 },
            1.0,
            1,
            Duration(10_000),
            false,
        ),
    ];
    for hazard in hazards {
        run_command(
            &mut session,
            "registrar peligro conocido de ruta",
            GameplayCommand::RegisterEnvironmentalHazard { hazard },
        )?;
    }
    let resolved_encounter = EncounterId::new(6_301);
    run_command(
        &mut session,
        "registrar encuentro resuelto de ruta",
        GameplayCommand::RegisterSpatialEncounter {
            encounter: SpatialEncounter::new(
                resolved_encounter,
                SpatialEncounterKind::Resource {
                    item: ItemId(1),
                    amount: 1,
                },
                Vec3::ZERO,
                2.0,
            ),
        },
    )?;
    run_command(
        &mut session,
        "activar encuentro resuelto de ruta",
        GameplayCommand::ActivateSpatialEncounter {
            encounter: resolved_encounter,
        },
    )?;
    run_command(
        &mut session,
        "resolver encuentro resuelto de ruta",
        GameplayCommand::ResolveSpatialEncounter {
            player,
            encounter: resolved_encounter,
        },
    )?;
    run_command(
        &mut session,
        "ocupar estaciÃ³n para evaluar riesgos",
        GameplayCommand::Interact {
            player,
            target: station,
        },
    )?;
    run_command(
        &mut session,
        "planificar ruta con riesgos conocidos",
        GameplayCommand::PlanAutopilot {
            player,
            pilot_station: station,
            origin,
            destination,
        },
    )?;
    let assessment = session
        .world()
        .companion_navigation_assessment(erick.entity)
        .map_err(|error| format!("evaluar riesgos geomÃ©tricos: {error:?}"))?
        .ok_or_else(|| "ERICK no produjo evaluaciÃ³n geomÃ©trica".to_owned())?;
    let near = assessment.route_risks.iter().find(|risk| {
        risk.source == gameplay::NavigationRouteRiskSource::Hazard(HazardId::new(6_301))
    });
    let far = assessment.route_risks.iter().find(|risk| {
        risk.source == gameplay::NavigationRouteRiskSource::Hazard(HazardId::new(6_302))
    });
    let beyond_start = assessment.route_risks.iter().find(|risk| {
        risk.source == gameplay::NavigationRouteRiskSource::Hazard(HazardId::new(6_303))
    });
    let inactive = assessment.route_risks.iter().find(|risk| {
        risk.source == gameplay::NavigationRouteRiskSource::Hazard(HazardId::new(6_304))
    });
    let resolved = assessment.route_risks.iter().find(|risk| {
        risk.source == gameplay::NavigationRouteRiskSource::Encounter(resolved_encounter)
    });
    let is_near = |risk: Option<&gameplay::NavigationRouteRisk>, distance: f64| {
        risk.is_some_and(|value| {
            matches!(
                value.classification,
                gameplay::NavigationRouteRiskClassification::NearSegment { segment: 0, distance: actual }
                    if (actual - distance).abs() < 0.000_001
            )
        })
    };
    let is_outside = |risk: Option<&gameplay::NavigationRouteRisk>, distance: f64| {
        risk.is_some_and(|value| {
            matches!(
                value.classification,
                gameplay::NavigationRouteRiskClassification::OutsideMargin { segment: 0, distance: actual }
                    if (actual - distance).abs() < 0.000_001
            )
        })
    };
    if !is_near(near, 1.0)
        || !is_outside(far, 4.0)
        || !is_outside(beyond_start, 3.0)
        || inactive.is_none_or(|risk| {
            risk.classification != gameplay::NavigationRouteRiskClassification::Resolved
        })
        || resolved.is_none_or(|risk| {
            risk.classification != gameplay::NavigationRouteRiskClassification::Resolved
        })
        || assessment.risks != vec![gameplay::NavigationRisk::KnownHazard(HazardId::new(6_301))]
        || assessment.recommendation
            != Some(gameplay::NavigationAdvice::InspectHazard(HazardId::new(6_301)))
        || assessment.recommendation_segment != Some(0)
    {
        return Err(format!("clasificaciÃ³n de riesgos inesperada: {assessment:?}"));
    }
    let before_repeat = session.world().save_state().serialize();
    let repeated = session
        .world()
        .companion_navigation_assessment(erick.entity)
        .map_err(|error| format!("repetir evaluaciÃ³n geomÃ©trica: {error:?}"))?;
    if repeated != Some(assessment.clone()) || session.world().save_state().serialize() != before_repeat {
        return Err("la evaluaciÃ³n geomÃ©trica no fue determinista o mutÃ³ el mundo".to_owned());
    }
    let saved = session.save_state();
    let mut restored = ProjectEchoSession::new();
    restored
        .restore_state(saved)
        .map_err(|error| format!("restaurar riesgos de ruta: {error}"))?;
    if restored
        .world()
        .companion_navigation_assessment(erick.entity)
        .map_err(|error| format!("evaluar riesgos restaurados: {error:?}"))?
        != Some(assessment)
    {
        return Err("la evaluaciÃ³n restaurada no fue equivalente".to_owned());
    }
    let mut degenerate = ProjectEchoSession::new();
    let degenerate_player = degenerate.ids.first_player;
    let degenerate_station = degenerate
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estaciÃ³n degenerada: {error:?}"))?;
    run_command(
        &mut degenerate,
        "crear ERICK para segmento degenerado",
        GameplayCommand::CreateCompanion {
            followed_player: degenerate_player,
            position: Vec3::ZERO,
        },
    )?;
    let degenerate_erick = degenerate
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK degenerado no quedÃ³ registrado".to_owned())?;
    for value in [
        Destination {
            id: DestinationId::new(6_311),
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        },
        Destination {
            id: DestinationId::new(6_312),
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        },
    ] {
        run_command(
            &mut degenerate,
            "registrar destino degenerado",
            GameplayCommand::RegisterDestination { destination: value },
        )?;
    }
    run_command(
        &mut degenerate,
        "registrar segmento degenerado",
        GameplayCommand::RegisterNavigationRoute {
            route: NavigationRoute {
                from: DestinationId::new(6_311),
                to: DestinationId::new(6_312),
                distance: 1.0,
                duration: Duration(1),
                fuel_cost: 1,
                state: NavigationRouteState::Available,
            },
        },
    )?;
    run_command(
        &mut degenerate,
        "ocupar estaciÃ³n degenerada",
        GameplayCommand::Interact {
            player: degenerate_player,
            target: degenerate_station,
        },
    )?;
    run_command(
        &mut degenerate,
        "planificar segmento degenerado",
        GameplayCommand::PlanAutopilot {
            player: degenerate_player,
            pilot_station: degenerate_station,
            origin: DestinationId::new(6_311),
            destination: DestinationId::new(6_312),
        },
    )?;
    run_command(
        &mut degenerate,
        "registrar peligro no evaluable",
        GameplayCommand::RegisterEnvironmentalHazard {
            hazard: EnvironmentalHazard::new(
                HazardId::new(6_313),
                gameplay::EnvironmentalHazardType::SpaceStorm,
                Vec3 { x: 1.0, y: 0.0, z: 0.0 },
                1.0,
                1,
                Duration(10_000),
                true,
            ),
        },
    )?;
    let degenerate_assessment = degenerate
        .world()
        .companion_navigation_assessment(degenerate_erick.entity)
        .map_err(|error| format!("evaluar segmento degenerado: {error:?}"))?
        .ok_or_else(|| "no hubo evaluaciÃ³n del segmento degenerado".to_owned())?;
    if !degenerate_assessment.route_risks.iter().any(|risk| {
        risk.source == gameplay::NavigationRouteRiskSource::Hazard(HazardId::new(6_313))
            && risk.classification == gameplay::NavigationRouteRiskClassification::NotEvaluable
    }) {
        return Err("el segmento degenerado produjo una clasificaciÃ³n engaÃ±osa".to_owned());
    }
    println!(
        "RIESGOS DE RUTA DE ERICK COMPLETADOS: cercano=segmento0, alejados=2, resueltos=2, restaurado=true, degenerado=no_evaluable"
    );
    Ok(())
}

fn run_erick_route_comparison_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let station = session
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estaciÃ³n para comparar rutas: {error:?}"))?;
    run_command(
        &mut session,
        "crear ERICK para comparar rutas",
        GameplayCommand::CreateCompanion {
            followed_player: player,
            position: Vec3::ZERO,
        },
    )?;
    let erick = session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK no quedÃ³ registrado para comparar rutas".to_owned())?;
    let origin = DestinationId::new(6_401);
    let destination = DestinationId::new(6_402);
    let safe_a = DestinationId::new(6_403);
    let safe_b = DestinationId::new(6_404);
    for value in [
        Destination {
            id: origin,
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        },
        Destination {
            id: destination,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        },
        Destination {
            id: safe_a,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 5.0, y: 10.0, z: 0.0 },
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        },
        Destination {
            id: safe_b,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 5.0, y: -10.0, z: 0.0 },
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        },
    ] {
        run_command(
            &mut session,
            "registrar destino de ruta alternativa",
            GameplayCommand::RegisterDestination { destination: value },
        )?;
    }
    let route = |from, to, distance, duration, fuel_cost| NavigationRoute {
        from,
        to,
        distance,
        duration: Duration(duration),
        fuel_cost,
        state: NavigationRouteState::Available,
    };
    for value in [
        route(origin, destination, 10.0, 4, 4),
        route(origin, safe_a, 6.0, 4, 2),
        route(safe_a, destination, 6.0, 4, 2),
        route(origin, safe_b, 6.0, 4, 2),
        route(safe_b, destination, 6.0, 4, 2),
    ] {
        run_command(
            &mut session,
            "registrar conexiÃ³n de ruta alternativa",
            GameplayCommand::RegisterNavigationRoute { route: value },
        )?;
    }
    run_command(
        &mut session,
        "ocupar estaciÃ³n para comparar rutas",
        GameplayCommand::Interact {
            player,
            target: station,
        },
    )?;
    run_command(
        &mut session,
        "planificar ruta actual con riesgo",
        GameplayCommand::PlanAutopilot {
            player,
            pilot_station: station,
            origin,
            destination,
        },
    )?;
    let hazard = HazardId::new(6_401);
    run_command(
        &mut session,
        "registrar peligro sobre ruta actual",
        GameplayCommand::RegisterEnvironmentalHazard {
            hazard: EnvironmentalHazard::new(
                hazard,
                gameplay::EnvironmentalHazardType::SpaceStorm,
                Vec3 { x: 5.0, y: 1.0, z: 0.0 },
                1.0,
                1,
                Duration(10_000),
                true,
            ),
        },
    )?;
    let encounter = EncounterId::new(6_402);
    run_command(
        &mut session,
        "registrar encuentro compartido por las rutas",
        GameplayCommand::RegisterSpatialEncounter {
            encounter: SpatialEncounter::new(
                encounter,
                SpatialEncounterKind::Resource {
                    item: ItemId(1),
                    amount: 1,
                },
                Vec3::ZERO,
                2.0,
            ),
        },
    )?;
    run_command(
        &mut session,
        "activar encuentro compartido por las rutas",
        GameplayCommand::ActivateSpatialEncounter { encounter },
    )?;
    let saved_before_queries = session.save_state();
    let comparison = session
        .world()
        .companion_navigation_comparison(erick.entity)
        .map_err(|error| format!("comparar rutas de ERICK: {error:?}"))?
        .ok_or_else(|| "ERICK no produjo comparaciÃ³n de rutas".to_owned())?;
    let recommendation = comparison
        .recommendation
        .as_ref()
        .ok_or_else(|| "no se recomendÃ³ una alternativa viable al riesgo conocido".to_owned())?;
    if comparison.current.route != vec![origin, destination]
        || comparison.current.distance != 10.0
        || comparison.current.eta != Duration(4)
        || comparison.current.fuel_required != 4
        || recommendation.current != vec![origin, destination]
        || recommendation.alternative != vec![origin, safe_a, destination]
        || recommendation.reason
            != gameplay::NavigationRecommendationReason::AvoidsKnownRisk(
                gameplay::NavigationRouteRiskSource::Hazard(hazard),
            )
        || recommendation.avoided_risk
            != Some(gameplay::NavigationRouteRiskSource::Hazard(hazard))
        || recommendation.alternative_eta != Duration(8)
        || recommendation.alternative_fuel != 4
        || comparison.alternatives.len() != 2
    {
        return Err(format!("comparaciÃ³n de rutas inesperada: {comparison:?}"));
    }
    let repeated = session
        .world()
        .companion_navigation_comparison(erick.entity)
        .map_err(|error| format!("repetir comparaciÃ³n de rutas: {error:?}"))?;
    if repeated != Some(comparison.clone())
        || session.save_state().serialize() != saved_before_queries.serialize()
    {
        return Err("comparar rutas produjo resultados no deterministas o mutaciones".to_owned());
    }
    run_command(
        &mut session,
        "resolver encuentro compartido por las rutas",
        GameplayCommand::ResolveSpatialEncounter { player, encounter },
    )?;
    let after_resolve = session
        .world()
        .companion_navigation_comparison(erick.entity)
        .map_err(|error| format!("actualizar comparaciÃ³n tras resolver riesgo: {error:?}"))?
        .ok_or_else(|| "la comparaciÃ³n desapareciÃ³ tras resolver un riesgo".to_owned())?;
    if after_resolve
        .current
        .risks
        .iter()
        .filter(|risk| risk.source == gameplay::NavigationRouteRiskSource::Encounter(encounter))
        .any(|risk| risk.classification != gameplay::NavigationRouteRiskClassification::Resolved)
        || after_resolve.recommendation != comparison.recommendation
    {
        return Err("la resoluciÃ³n no actualizÃ³ correctamente la comparaciÃ³n".to_owned());
    }
    run_command(
        &mut session,
        "deshabilitar ERICK para validar bloqueo de asistencia",
        GameplayCommand::SetCompanionBehavior {
            companion: erick.entity,
            behavior: gameplay::CompanionBehavior::Unavailable,
        },
    )?;
    if session
        .world()
        .companion_navigation_comparison(erick.entity)
        .map_err(|error| format!("consultar ERICK no disponible: {error:?}"))?
        .is_some()
    {
        return Err("ERICK no disponible mantuvo una recomendaciÃ³n de ruta".to_owned());
    }
    run_command(
        &mut session,
        "reactivar ERICK para continuar comparaciÃ³n",
        GameplayCommand::SetCompanionBehavior {
            companion: erick.entity,
            behavior: gameplay::CompanionBehavior::Accompany,
        },
    )?;
    let ship = session.ids.first_ship;
    let fuel_to_remove = session
        .world()
        .ship_fuel(ship)
        .ok_or_else(|| "faltan recursos para invalidar alternativas".to_owned())?
        .current();
    run_command(
        &mut session,
        "agotar combustible para invalidar alternativas",
        GameplayCommand::RemoveShipFuel {
            ship,
            amount: fuel_to_remove,
        },
    )?;
    let no_viable_alternative = session
        .world()
        .companion_navigation_comparison(erick.entity)
        .map_err(|error| format!("evaluar alternativa inviable: {error:?}"))?
        .ok_or_else(|| "la comparaciÃ³n no estuvo disponible con ruta planificada".to_owned())?;
    if no_viable_alternative.recommendation.is_some()
        || no_viable_alternative.alternatives.iter().any(|route| route.viable)
    {
        return Err("se recomendÃ³ una ruta sin combustible suficiente".to_owned());
    }
    let restored_state = saved_before_queries;
    let mut restored = ProjectEchoSession::new();
    restored
        .restore_state(restored_state)
        .map_err(|error| format!("restaurar comparaciÃ³n de rutas: {error}"))?;
    if restored
        .world()
        .companion_navigation_comparison(erick.entity)
        .map_err(|error| format!("comparar rutas restauradas: {error:?}"))?
        != Some(comparison.clone())
    {
        return Err("la comparaciÃ³n restaurada no fue equivalente".to_owned());
    }
    run_command(
        &mut restored,
        "iniciar autopilot para comprobar estado en trÃ¡nsito",
        GameplayCommand::StartAutopilot {
            player,
            pilot_station: station,
        },
    )?;
    let in_transit = restored
        .world()
        .companion_navigation_comparison(erick.entity)
        .map_err(|error| format!("comparar ruta en trÃ¡nsito: {error:?}"))?
        .ok_or_else(|| "no se pudo comparar la ruta en trÃ¡nsito".to_owned())?;
    if in_transit.recommendation.is_some() {
        return Err("ERICK intentÃ³ cambiar una ruta ya iniciada".to_owned());
    }
    run_command(
        &mut restored,
        "desactivar jugador para bloquear asistencia de rutas",
        GameplayCommand::SetPlayerActive {
            player,
            active: false,
        },
    )?;
    if restored
        .world()
        .companion_navigation_comparison(erick.entity)
        .map_err(|error| format!("consultar ERICK con jugador inactivo: {error:?}"))?
        .is_some()
    {
        return Err("ERICK mantuvo asistencia accionable con jugador inactivo".to_owned());
    }
    println!(
        "COMPARACION DE RUTAS DE ERICK COMPLETADA: actual={:?}, alternativa={:?}, riesgo_evitado={:?}, empate_determinista=true",
        recommendation.current, recommendation.alternative, recommendation.avoided_risk
    );
    Ok(())
}

fn run_erick_operational_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let ship = session.ids.first_ship;
    let engineering = session
        .create_station(Vec3::ZERO, 2.0, StationType::Engineering)
        .map_err(|error| format!("crear estación de ingeniería de ERICK: {error:?}"))?;
    let piloting = session
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estación de pilotaje de ERICK: {error:?}"))?;
    run_command(
        &mut session,
        "crear ERICK para evaluar mantenimiento",
        GameplayCommand::CreateCompanion {
            followed_player: player,
            position: Vec3::ZERO,
        },
    )?;
    let erick = session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK no quedó registrado para evaluar mantenimiento".to_owned())?;
    let initial = session
        .world()
        .companion_operational_assessment(erick.entity)
        .map_err(|error| format!("evaluar estado operativo inicial: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación operativa inicial".to_owned())?;
    if initial.hull != gameplay::HullCondition::Normal
        || initial.fuel != gameplay::FuelCondition::Unknown
        || initial.priority != gameplay::MaintenancePriority::Normal
        || initial.recommendation.is_some()
    {
        return Err(format!("estado operativo inicial inesperado: {initial:?}"));
    }
    let initial_state = session.save_state();
    let repeated_initial = session
        .world()
        .companion_operational_assessment(erick.entity)
        .map_err(|error| format!("repetir estado operativo inicial: {error:?}"))?;
    if repeated_initial != Some(initial)
        || session.save_state().serialize() != initial_state.serialize()
    {
        return Err("consultar el estado operativo inicial produjo mutaciones".to_owned());
    }
    let origin = DestinationId::new(6_501);
    let destination = DestinationId::new(6_502);
    for value in [
        Destination {
            id: origin,
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(10),
            fuel_cost: 10,
            discovery: DiscoveryState::Discovered,
        },
        Destination {
            id: destination,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
            travel_duration: Duration(10),
            fuel_cost: 10,
            discovery: DiscoveryState::Discovered,
        },
    ] {
        run_command(
            &mut session,
            "registrar destino para evaluación operativa",
            GameplayCommand::RegisterDestination { destination: value },
        )?;
    }
    run_command(
        &mut session,
        "registrar ruta para evaluación operativa",
        GameplayCommand::RegisterNavigationRoute {
            route: NavigationRoute {
                from: origin,
                to: destination,
                distance: 10.0,
                duration: Duration(10),
                fuel_cost: 10,
                state: NavigationRouteState::Available,
            },
        },
    )?;
    run_command(
        &mut session,
        "ocupar estación de pilotaje para ERICK",
        GameplayCommand::Interact {
            player,
            target: piloting,
        },
    )?;
    run_command(
        &mut session,
        "planificar ruta para ERICK",
        GameplayCommand::PlanAutopilot {
            player,
            pilot_station: piloting,
            origin,
            destination,
        },
    )?;
    let hazard = HazardId::new(6_501);
    run_command(
        &mut session,
        "registrar peligro conocido para ERICK",
        GameplayCommand::RegisterEnvironmentalHazard {
            hazard: EnvironmentalHazard::new(
                hazard,
                gameplay::EnvironmentalHazardType::SpaceStorm,
                Vec3 { x: 5.0, y: 0.0, z: 0.0 },
                1.0,
                1,
                Duration(10_000),
                true,
            ),
        },
    )?;
    let planned = session
        .world()
        .companion_operational_assessment(erick.entity)
        .map_err(|error| format!("evaluar ruta y riesgo de ERICK: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación operativa planificada".to_owned())?;
    if planned.fuel != gameplay::FuelCondition::Sufficient
        || planned.fuel_available != Some(60)
        || planned.fuel_required != Some(10)
        || planned.priority != gameplay::MaintenancePriority::Normal
        || planned.recommendation.is_some()
        || planned.navigation.as_ref().and_then(|value| value.recommendation)
            != Some(gameplay::NavigationAdvice::InspectHazard(hazard))
    {
        return Err(format!("evaluación planificada inesperada: {planned:?}"));
    }
    run_command(
        &mut session,
        "dañar nave para probar estado crítico",
        GameplayCommand::DamageShip { ship, amount: 80 },
    )?;
    let damaged = session
        .world()
        .companion_operational_assessment(erick.entity)
        .map_err(|error| format!("evaluar integridad reducida: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación tras daño".to_owned())?;
    if damaged.integrity != 20
        || damaged.hull != gameplay::HullCondition::Critical
        || damaged.priority != gameplay::MaintenancePriority::Critical
        || damaged.recommendation != Some(gameplay::MaintenanceRecommendation::RepairShip { ship })
    {
        return Err(format!("estado tras daño inesperado: {damaged:?}"));
    }
    run_command(
        &mut session,
        "añadir materiales reales de reparación",
        GameplayCommand::AddInventoryItem {
            player,
            item: ItemId(1),
            amount: 4,
        },
    )?;
    run_command(
        &mut session,
        "ocupar estación de ingeniería para reparar",
        GameplayCommand::Interact {
            player,
            target: engineering,
        },
    )?;
    for _ in 0..4 {
        run_command(
            &mut session,
            "reparar nave mediante estación real",
            GameplayCommand::RepairShipAtStation {
                player,
                ship,
                station: engineering,
            },
        )?;
    }
    let repaired = session
        .world()
        .companion_operational_assessment(erick.entity)
        .map_err(|error| format!("evaluar nave reparada: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación tras reparar".to_owned())?;
    let repaired_material = session
        .world()
        .inventory(player)
        .map(|inventory| inventory.quantity(ItemId(1)))
        .unwrap_or_default();
    if repaired.integrity != 100
        || repaired.hull != gameplay::HullCondition::Normal
        || repaired.recommendation.is_some()
        || repaired_material != 0
    {
        return Err(format!(
            "reparación o consumo de material inesperado: assessment={repaired:?}, material={repaired_material}"
        ));
    }
    run_command(
        &mut session,
        "retirar combustible para margen ajustado",
        GameplayCommand::RemoveShipFuel { ship, amount: 50 },
    )?;
    let tight = session
        .world()
        .companion_operational_assessment(erick.entity)
        .map_err(|error| format!("evaluar margen ajustado: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación con margen ajustado".to_owned())?;
    if tight.fuel != gameplay::FuelCondition::Tight
        || tight.priority != gameplay::MaintenancePriority::High
        || tight.recommendation
            != Some(gameplay::MaintenanceRecommendation::ManageFuel {
                ship,
                required: 10,
                available: 10,
            })
    {
        return Err(format!("margen de combustible inesperado: {tight:?}"));
    }
    run_command(
        &mut session,
        "añadir combustible para recuperar margen",
        GameplayCommand::AddShipFuel { ship, amount: 1 },
    )?;
    let sufficient = session
        .world()
        .companion_operational_assessment(erick.entity)
        .map_err(|error| format!("evaluar combustible suficiente: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación con combustible suficiente".to_owned())?;
    if sufficient.fuel != gameplay::FuelCondition::Sufficient
        || sufficient.recommendation.is_some()
    {
        return Err(format!("combustible suficiente produjo alerta: {sufficient:?}"));
    }
    let saved = session.save_state();
    run_command(
        &mut session,
        "agotar combustible para invalidar ruta",
        GameplayCommand::RemoveShipFuel { ship, amount: 11 },
    )?;
    let insufficient = session
        .world()
        .companion_operational_assessment(erick.entity)
        .map_err(|error| format!("evaluar combustible insuficiente: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación sin combustible".to_owned())?;
    if insufficient.fuel != gameplay::FuelCondition::Insufficient
        || insufficient.priority != gameplay::MaintenancePriority::Critical
        || insufficient.recommendation
            != Some(gameplay::MaintenanceRecommendation::ManageFuel {
                ship,
                required: 10,
                available: 0,
            })
        || insufficient
            .route_comparison
            .as_ref()
            .is_some_and(|comparison| comparison.current.viable)
    {
        return Err(format!("combustible insuficiente inesperado: {insufficient:?}"));
    }
    let mut restored = ProjectEchoSession::new();
    restored
        .restore_state(saved)
        .map_err(|error| format!("restaurar estado operativo de ERICK: {error}"))?;
    let restored_assessment = restored
        .world()
        .companion_operational_assessment(erick.entity)
        .map_err(|error| format!("evaluar estado restaurado de ERICK: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación restaurada".to_owned())?;
    if restored_assessment != sufficient {
        return Err("la evaluación restaurada no conservó el estado operativo".to_owned());
    }
    run_command(
        &mut restored,
        "deshabilitar ERICK para validar disponibilidad",
        GameplayCommand::SetCompanionBehavior {
            companion: erick.entity,
            behavior: gameplay::CompanionBehavior::Unavailable,
        },
    )?;
    if restored
        .world()
        .companion_operational_assessment(erick.entity)
        .map_err(|error| format!("consultar ERICK no disponible: {error:?}"))?
        .is_some()
    {
        return Err("ERICK no disponible mantuvo evaluación operativa".to_owned());
    }
    run_command(
        &mut restored,
        "reactivar ERICK para validar jugador inactivo",
        GameplayCommand::SetCompanionBehavior {
            companion: erick.entity,
            behavior: gameplay::CompanionBehavior::Accompany,
        },
    )?;
    run_command(
        &mut restored,
        "desactivar jugador para bloquear asistencia",
        GameplayCommand::SetPlayerActive {
            player,
            active: false,
        },
    )?;
    if restored
        .world()
        .companion_operational_assessment(erick.entity)
        .map_err(|error| format!("consultar ERICK con jugador inactivo: {error:?}"))?
        .is_some()
    {
        return Err("ERICK mantuvo evaluación con jugador inactivo".to_owned());
    }
    println!(
        "MANTENIMIENTO DE ERICK COMPLETADO: integridad={}, combustible_final={}, prioridad_riesgo={:?}, reparacion_jugador=true, restaurado=true",
        sufficient.integrity,
        sufficient.fuel_available.unwrap_or_default(),
        insufficient.priority
    );
    Ok(())
}

fn run_erick_exploration_coordination_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let destination_origin = DestinationId::new(6_601);
    let destination_target = DestinationId::new(6_602);
    let mission = gameplay::types::MissionId(66);
    let objective = gameplay::types::ObjectiveId(6_601);
    let station = session
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estación para coordinación: {error:?}"))?;
    session
        .world_mut()
        .register_mission(
            gameplay::Mission::new(
                mission,
                "Objetivo de exploración de ERICK",
                vec![gameplay::Objective::new(
                    objective,
                    mission,
                    "Alcanzar el destino de exploración",
                    true,
                    gameplay::ObjectiveTrigger::DestinationArrived {
                        destination: destination_target,
                    },
                )],
            )
            .map_err(|error| format!("definir misión de coordinación: {error:?}"))?,
        )
        .map_err(|error| format!("registrar misión de coordinación: {error:?}"))?;
    for destination in [
        Destination {
            id: destination_origin,
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 10,
            discovery: DiscoveryState::Discovered,
        },
        Destination {
            id: destination_target,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
            travel_duration: Duration(1),
            fuel_cost: 10,
            discovery: DiscoveryState::Discovered,
        },
    ] {
        run_command(
            &mut session,
            "registrar destino de coordinación",
            GameplayCommand::RegisterDestination { destination },
        )?;
    }
    run_command(
        &mut session,
        "registrar ruta de coordinación",
        GameplayCommand::RegisterNavigationRoute {
            route: NavigationRoute {
                from: destination_origin,
                to: destination_target,
                distance: 10.0,
                duration: Duration(1),
                fuel_cost: 10,
                state: NavigationRouteState::Available,
            },
        },
    )?;
    let derelict = DerelictId::new(6_601);
    run_command(
        &mut session,
        "registrar derelicto pendiente de coordinación",
        GameplayCommand::RegisterDerelict {
            derelict: Derelict::new(derelict, Vec3::ZERO, 2.0).with_rewards(
                vec![DerelictReward {
                    item: ItemId(1),
                    amount: 1,
                }],
                RewardDistributionPolicy::Shared,
            ),
        },
    )?;
    run_command(
        &mut session,
        "descubrir derelicto para coordinación",
        GameplayCommand::DiscoverDerelict { player, derelict },
    )?;
    let encounter = EncounterId::new(6_601);
    run_command(
        &mut session,
        "registrar encuentro pendiente de coordinación",
        GameplayCommand::RegisterSpatialEncounter {
            encounter: SpatialEncounter::new(
                encounter,
                SpatialEncounterKind::Resource {
                    item: ItemId(1),
                    amount: 1,
                },
                Vec3::ZERO,
                2.0,
            ),
        },
    )?;
    run_command(
        &mut session,
        "activar encuentro de coordinación",
        GameplayCommand::ActivateSpatialEncounter { encounter },
    )?;
    run_command(
        &mut session,
        "registrar mundo de recursos de coordinación",
        GameplayCommand::RegisterFunctionalWorld {
            world: FunctionalWorld::new(
                destination_target,
                2.0,
                vec![WorldActivity::Resource(ResourceSite {
                    item: ItemId(1),
                    available: 1,
                    extraction_amount: 1,
                })],
            ),
        },
    )?;
    run_command(
        &mut session,
        "desbloquear misión de coordinación",
        GameplayCommand::UnlockMission { mission },
    )?;
    run_command(
        &mut session,
        "iniciar misión de coordinación",
        GameplayCommand::StartMission { mission },
    )?;
    run_command(
        &mut session,
        "crear ERICK para coordinar exploración",
        GameplayCommand::CreateCompanion {
            followed_player: player,
            position: Vec3::ZERO,
        },
    )?;
    let erick = session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK no quedó registrado para coordinar exploración".to_owned())?;
    let initial = session
        .world()
        .companion_exploration_assessment(erick.entity)
        .map_err(|error| format!("consultar coordinación inicial: {error:?}"))?
        .ok_or_else(|| "no hubo coordinación inicial".to_owned())?;
    let mission_recommendation = initial
        .primary
        .as_ref()
        .ok_or_else(|| "ERICK no identificó el objetivo de misión".to_owned())?;
    if mission_recommendation.category != gameplay::ExplorationTargetCategory::MissionObjective
        || mission_recommendation.mission != Some(mission)
        || mission_recommendation.objective != Some(objective)
        || mission_recommendation.destination != Some(destination_target)
        || !mission_recommendation.accessible
        || mission_recommendation.action
            != gameplay::ExplorationAction::Guidance(gameplay::GuidanceAction::TravelTo(
                destination_target,
            ))
        || !initial.alternatives.iter().any(|value| {
            value.category == gameplay::ExplorationTargetCategory::Encounter
                && value.action == gameplay::ExplorationAction::ResolveEncounter(encounter)
        })
    {
        return Err(format!("recomendación de misión inesperada: {initial:?}"));
    }
    let saved_initial = session.save_state();
    let repeated = session
        .world()
        .companion_exploration_assessment(erick.entity)
        .map_err(|error| format!("repetir coordinación inicial: {error:?}"))?;
    if repeated != Some(initial.clone())
        || session.save_state().serialize() != saved_initial.serialize()
    {
        return Err("la coordinación inicial no fue determinista o mutó el mundo".to_owned());
    }
    let ship = session.ids.first_ship;
    run_command(
        &mut session,
        "agotar combustible para probar ruta inviable",
        GameplayCommand::RemoveShipFuel {
            ship,
            amount: 60,
        },
    )?;
    let infeasible = session
        .world()
        .companion_exploration_assessment(erick.entity)
        .map_err(|error| format!("consultar ruta inviable: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación con ruta inviable".to_owned())?;
    if infeasible.primary.as_ref().is_some_and(|value| {
        value.mission == Some(mission) && value.destination == Some(destination_target)
    }) || infeasible
        .alternatives
        .iter()
        .any(|value| value.mission == Some(mission) && !value.accessible)
    {
        return Err("ERICK recomendó una misión sin ruta viable".to_owned());
    }
    session
        .restore_state(saved_initial)
        .map_err(|error| format!("restaurar coordinación inicial: {error}"))?;
    run_command(
        &mut session,
        "resolver encuentro para actualizar coordinación",
        GameplayCommand::ResolveSpatialEncounter { player, encounter },
    )?;
    let after_encounter = session
        .world()
        .companion_exploration_assessment(erick.entity)
        .map_err(|error| format!("consultar coordinación tras encuentro: {error:?}"))?
        .ok_or_else(|| "no hubo coordinación tras resolver encuentro".to_owned())?;
    if after_encounter
        .alternatives
        .iter()
        .any(|value| value.action == gameplay::ExplorationAction::ResolveEncounter(encounter))
    {
        return Err("el encuentro resuelto siguió siendo recomendado".to_owned());
    }
    run_command(
        &mut session,
        "ocupar estación para ejecutar objetivo recomendado",
        GameplayCommand::Interact { player, target: station },
    )?;
    run_command(
        &mut session,
        "planificar destino recomendado",
        GameplayCommand::PlanAutopilot {
            player,
            pilot_station: station,
            origin: destination_origin,
            destination: destination_target,
        },
    )?;
    run_command(
        &mut session,
        "iniciar viaje recomendado por el jugador",
        GameplayCommand::StartAutopilot {
            player,
            pilot_station: station,
        },
    )?;
    run_command(
        &mut session,
        "avanzar tiempo lógico hasta el destino",
        GameplayCommand::AdvanceTime { delta: Duration(1) },
    )?;
    run_command(
        &mut session,
        "acercar jugador al destino mediante movimiento real",
        GameplayCommand::MovePlayer {
            player,
            direction: Vec3 {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            delta: Duration(2_000),
        },
    )?;
    let after_arrival = session
        .world()
        .companion_exploration_assessment(erick.entity)
        .map_err(|error| format!("consultar coordinación tras llegada: {error:?}"))?
        .ok_or_else(|| "no hubo coordinación tras llegada".to_owned())?;
    if after_arrival.primary.as_ref().is_some_and(|value| {
        value.objective == Some(objective)
    }) || !after_arrival.primary.as_ref().is_some_and(|value| {
        value.category == gameplay::ExplorationTargetCategory::WorldActivity
            && value.destination == Some(destination_target)
    }) {
        return Err(format!("el objetivo completado o el mundo no actualizaron la recomendación: {after_arrival:?}"));
    }
    let before_extract = session.save_state();
    run_command(
        &mut session,
        "extraer recurso del objetivo recomendado",
        GameplayCommand::ExtractWorldResource {
            player,
            destination: destination_target,
        },
    )?;
    let after_extract = session
        .world()
        .companion_exploration_assessment(erick.entity)
        .map_err(|error| format!("consultar actividad agotada: {error:?}"))?
        .ok_or_else(|| "no hubo coordinación tras agotar actividad".to_owned())?;
    if after_extract
        .primary
        .as_ref()
        .is_some_and(|value| value.destination == Some(destination_target))
        || after_extract
            .alternatives
            .iter()
            .any(|value| value.destination == Some(destination_target))
    {
        return Err("la actividad agotada siguió siendo recomendada".to_owned());
    }
    let saved_final = session.save_state();
    let saved_final_text = saved_final.serialize();
    let mut restored = ProjectEchoSession::new();
    restored
        .restore_state(saved_final)
        .map_err(|error| format!("restaurar coordinación final: {error}"))?;
    let restored_assessment = restored
        .world()
        .companion_exploration_assessment(erick.entity)
        .map_err(|error| format!("consultar coordinación restaurada: {error:?}"))?;
    if restored_assessment != Some(after_extract.clone())
        || before_extract.serialize() == saved_final_text
    {
        return Err("la restauración o el consumo de recurso no fueron observables correctamente".to_owned());
    }
    run_command(
        &mut restored,
        "deshabilitar ERICK para validar asistencia",
        GameplayCommand::SetCompanionBehavior {
            companion: erick.entity,
            behavior: gameplay::CompanionBehavior::Unavailable,
        },
    )?;
    if restored
        .world()
        .companion_exploration_assessment(erick.entity)
        .map_err(|error| format!("consultar ERICK no disponible: {error:?}"))?
        .is_some()
    {
        return Err("ERICK no disponible conservó asistencia accionable".to_owned());
    }
    println!(
        "COORDINACION DE EXPLORACION DE ERICK COMPLETADA: mision={mission:?}, objetivo={objective:?}, destino={destination_target:?}, derelicto={derelict:?}, encuentro_resuelto=true, actividad_agotada=true, restaurado=true"
    );
    Ok(())
}

fn run_erick_preparation_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let ship = session.ids.first_ship;
    let origin = DestinationId::new(6_701);
    let destination = DestinationId::new(6_702);
    let mission = gameplay::types::MissionId(67);
    let objective = gameplay::types::ObjectiveId(6_701);
    for value in [
        Destination {
            id: origin,
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 10,
            discovery: DiscoveryState::Discovered,
        },
        Destination {
            id: destination,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
            travel_duration: Duration(1),
            fuel_cost: 10,
            discovery: DiscoveryState::Discovered,
        },
    ] {
        run_command(
            &mut session,
            "registrar destino de preparación",
            GameplayCommand::RegisterDestination { destination: value },
        )?;
    }
    run_command(
        &mut session,
        "registrar ruta de preparación",
        GameplayCommand::RegisterNavigationRoute {
            route: NavigationRoute {
                from: origin,
                to: destination,
                distance: 10.0,
                duration: Duration(1),
                fuel_cost: 10,
                state: NavigationRouteState::Available,
            },
        },
    )?;
    session
        .world_mut()
        .register_mission(
            gameplay::Mission::new(
                mission,
                "Preparación de misión de ERICK",
                vec![gameplay::Objective::new(
                    objective,
                    mission,
                    "Preparar viaje",
                    true,
                    gameplay::ObjectiveTrigger::DestinationArrived { destination },
                )],
            )
            .map_err(|error| format!("definir misión de preparación: {error:?}"))?,
        )
        .map_err(|error| format!("registrar misión de preparación: {error:?}"))?;
    run_command(
        &mut session,
        "desbloquear misión de preparación",
        GameplayCommand::UnlockMission { mission },
    )?;
    run_command(
        &mut session,
        "iniciar misión de preparación",
        GameplayCommand::StartMission { mission },
    )?;
    run_command(
        &mut session,
        "crear ERICK para evaluar preparación",
        GameplayCommand::CreateCompanion {
            followed_player: player,
            position: Vec3::ZERO,
        },
    )?;
    let erick = session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK no quedó registrado para preparación".to_owned())?;
    let complete = session
        .world()
        .companion_preparation_assessment(erick.entity)
        .map_err(|error| format!("evaluar preparación completa: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación de preparación completa".to_owned())?;
    if !complete.prepared
        || complete.priority != gameplay::PreparationPriority::Normal
        || complete.reason != gameplay::PreparationReason::Ready
        || !complete
            .satisfied_requirements
            .contains(&gameplay::PreparationRequirement::Fuel {
                required: 10,
                available: 60,
            })
    {
        return Err(format!("preparación completa inesperada: {complete:?}"));
    }
    let complete_snapshot = session.save_state();
    let repeated = session
        .world()
        .companion_preparation_assessment(erick.entity)
        .map_err(|error| format!("repetir preparación completa: {error:?}"))?;
    if repeated != Some(complete.clone())
        || session.save_state().serialize() != complete_snapshot.serialize()
    {
        return Err("consultar preparación produjo mutaciones o no fue determinista".to_owned());
    }
    run_command(
        &mut session,
        "reducir combustible para preparación insuficiente",
        GameplayCommand::RemoveShipFuel { ship, amount: 55 },
    )?;
    let fuel_missing = session
        .world()
        .companion_preparation_assessment(erick.entity)
        .map_err(|error| format!("evaluar combustible insuficiente: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación con combustible insuficiente".to_owned())?;
    if fuel_missing.prepared
        || fuel_missing.priority != gameplay::PreparationPriority::Critical
        || fuel_missing.reason != gameplay::PreparationReason::FuelShortage
        || fuel_missing.action != Some(gameplay::PreparationAction::PlayerRequired)
        || !fuel_missing
            .missing_requirements
            .contains(&gameplay::PreparationRequirement::Fuel {
                required: 10,
                available: 5,
            })
    {
        return Err(format!("preparación de combustible inesperada: {fuel_missing:?}"));
    }
    run_command(
        &mut session,
        "reponer combustible mediante API existente",
        GameplayCommand::AddShipFuel { ship, amount: 55 },
    )?;
    let collectible = session
        .world_mut()
        .create_collectible(ItemId(1), 1, Vec3::ZERO, 2.0)
        .map_err(|error| format!("crear recompensa pendiente: {error:?}"))?;
    run_command(
        &mut session,
        "llenar inventario para probar capacidad",
        GameplayCommand::AddInventoryItem {
            player,
            item: ItemId(1),
            amount: 20,
        },
    )?;
    let inventory_missing = session
        .world()
        .companion_preparation_assessment(erick.entity)
        .map_err(|error| format!("evaluar capacidad de recompensa: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación de capacidad".to_owned())?;
    if inventory_missing.prepared
        || inventory_missing.reason != gameplay::PreparationReason::InventoryCapacity
        || inventory_missing.action != Some(gameplay::PreparationAction::FreeInventory)
        || !inventory_missing
            .missing_requirements
            .contains(&gameplay::PreparationRequirement::InventoryCapacity {
                required: 1,
                available: 0,
            })
        || session.world().collectible(collectible).is_none_or(|item| !item.active)
    {
        return Err(format!("preparación de inventario inesperada: {inventory_missing:?}"));
    }
    run_command(
        &mut session,
        "liberar capacidad antes de recoger recompensa",
        GameplayCommand::RemoveInventoryItem {
            player,
            item: ItemId(1),
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "recoger recompensa pendiente con jugador",
        GameplayCommand::CollectItem {
            player,
            collectible,
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "crear enemigo para evaluar munición",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let enemy = session
        .world()
        .enemies()
        .next()
        .map(|enemy| enemy.entity)
        .ok_or_else(|| "la creación del enemigo no registró entidad".to_owned())?;
    run_command(
        &mut session,
        "equipar arma con munición real",
        GameplayCommand::EquipPlayerWeapon {
            player,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                15,
                10,
                Duration(1_000),
                WeaponAmmunition {
                    item: ItemId(2),
                    amount_per_attack: 1,
                },
            ),
        },
    )?;
    let ammunition_missing = session
        .world()
        .companion_preparation_assessment(erick.entity)
        .map_err(|error| format!("evaluar munición insuficiente: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación de munición".to_owned())?;
    if ammunition_missing.prepared
        || ammunition_missing.reason != gameplay::PreparationReason::AmmunitionShortage
        || ammunition_missing.action != Some(gameplay::PreparationAction::PrepareCombat(enemy))
    {
        return Err(format!("preparación de combate inesperada: {ammunition_missing:?}"));
    }
    run_command(
        &mut session,
        "liberar espacio para munición",
        GameplayCommand::RemoveInventoryItem {
            player,
            item: ItemId(1),
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "añadir munición real",
        GameplayCommand::AddInventoryItem {
            player,
            item: ItemId(2),
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "dañar nave para evaluar reparación",
        GameplayCommand::DamageShip { ship, amount: 80 },
    )?;
    run_command(
        &mut session,
        "retirar material para crear carencia de reparación",
        GameplayCommand::RemoveInventoryItem {
            player,
            item: ItemId(1),
            amount: 19,
        },
    )?;
    let repair_missing = session
        .world()
        .companion_preparation_assessment(erick.entity)
        .map_err(|error| format!("evaluar reparación insuficiente: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación de reparación".to_owned())?;
    if repair_missing.prepared
        || repair_missing.reason != gameplay::PreparationReason::RepairMaterialShortage
        || repair_missing.priority != gameplay::PreparationPriority::Critical
    {
        return Err(format!("preparación de reparación inesperada: {repair_missing:?}"));
    }
    let engineering = session
        .create_station(Vec3::ZERO, 2.0, StationType::Engineering)
        .map_err(|error| format!("crear estación de reparación: {error:?}"))?;
    run_command(
        &mut session,
        "añadir material real para reparar",
        GameplayCommand::AddInventoryItem {
            player,
            item: ItemId(1),
            amount: 1,
        },
    )?;
    run_command(
        &mut session,
        "ocupar estación de reparación",
        GameplayCommand::Interact {
            player,
            target: engineering,
        },
    )?;
    run_command(
        &mut session,
        "reparar nave con acción del jugador",
        GameplayCommand::RepairShipAtStation {
            player,
            ship,
            station: engineering,
        },
    )?;
    let repair_ready = session
        .world()
        .companion_preparation_assessment(erick.entity)
        .map_err(|error| format!("evaluar reparación aplicada: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación tras reparación".to_owned())?;
    if !repair_ready.prepared {
        return Err(format!("la reparación real no actualizó preparación: {repair_ready:?}"));
    }
    let before_queries = session.save_state();
    let repeated_missing = session
        .world()
        .companion_preparation_assessment(erick.entity)
        .map_err(|error| format!("repetir preparación insuficiente: {error:?}"))?;
    if repeated_missing != Some(repair_ready)
        || session.save_state().serialize() != before_queries.serialize()
    {
        return Err("la consulta repetida de preparación no fue determinista".to_owned());
    }
    let mut activity_session = ProjectEchoSession::new();
    let activity_player = activity_session.ids.first_player;
    activity_session
        .world_mut()
        .register_destination(Destination {
            id: DestinationId::new(6_703),
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 1,
            discovery: DiscoveryState::Discovered,
        })
        .map_err(|error| format!("registrar destino de actividad: {error:?}"))?;
    run_command(
        &mut activity_session,
        "registrar actividad de recursos",
        GameplayCommand::RegisterFunctionalWorld {
            world: FunctionalWorld::new(
                DestinationId::new(6_703),
                2.0,
                vec![WorldActivity::Resource(ResourceSite {
                    item: ItemId(1),
                    available: 1,
                    extraction_amount: 1,
                })],
            ),
        },
    )?;
    run_command(
        &mut activity_session,
        "crear ERICK para actividad",
        GameplayCommand::CreateCompanion {
            followed_player: activity_player,
            position: Vec3::ZERO,
        },
    )?;
    let activity_erick = activity_session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK no quedó registrado para actividad".to_owned())?;
    let activity_ready = activity_session
        .world()
        .companion_preparation_assessment(activity_erick.entity)
        .map_err(|error| format!("evaluar actividad preparada: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación de actividad".to_owned())?;
    if !activity_ready.prepared
        || activity_ready.exploration.as_ref().is_none_or(|target| {
            target.destination != Some(DestinationId::new(6_703))
        })
    {
        return Err(format!("actividad preparada inesperada: {activity_ready:?}"));
    }
    let activity_snapshot = activity_session.save_state();
    let repeated_activity = activity_session
        .world()
        .companion_preparation_assessment(activity_erick.entity)
        .map_err(|error| format!("repetir evaluación de actividad: {error:?}"))?;
    if repeated_activity != Some(activity_ready.clone())
        || activity_session.save_state().serialize() != activity_snapshot.serialize()
    {
        return Err("evaluar actividad produjo una mutación".to_owned());
    }
    run_command(
        &mut activity_session,
        "llenar inventario antes de extraer recurso",
        GameplayCommand::AddInventoryItem {
            player: activity_player,
            item: ItemId(1),
            amount: 20,
        },
    )?;
    let activity_blocked = activity_session
        .world()
        .companion_preparation_assessment(activity_erick.entity)
        .map_err(|error| format!("evaluar capacidad de actividad: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación bloqueada de actividad".to_owned())?;
    if activity_blocked.prepared
        || activity_blocked.reason != gameplay::PreparationReason::InventoryCapacity
    {
        return Err(format!("actividad bloqueada inesperada: {activity_blocked:?}"));
    }
    run_command(
        &mut activity_session,
        "vaciar inventario antes de extracción real",
        GameplayCommand::RemoveInventoryItem {
            player: activity_player,
            item: ItemId(1),
            amount: 20,
        },
    )?;
    run_command(
        &mut activity_session,
        "extraer recurso con jugador",
        GameplayCommand::ExtractWorldResource {
            player: activity_player,
            destination: DestinationId::new(6_703),
        },
    )?;
    let activity_exhausted = activity_session
        .world()
        .companion_preparation_assessment(activity_erick.entity)
        .map_err(|error| format!("evaluar actividad agotada: {error:?}"))?
        .ok_or_else(|| "no hubo evaluación tras agotar actividad".to_owned())?;
    if activity_exhausted.exploration.is_some() || !activity_exhausted.prepared {
        return Err(format!("actividad agotada no actualizó preparación: {activity_exhausted:?}"));
    }
    let saved_activity = activity_session.save_state();
    let mut restored = ProjectEchoSession::new();
    restored
        .restore_state(saved_activity)
        .map_err(|error| format!("restaurar preparación de actividad: {error}"))?;
    let restored_activity = restored
        .world()
        .companion_preparation_assessment(activity_erick.entity)
        .map_err(|error| format!("evaluar preparación restaurada: {error:?}"))?;
    if restored_activity != Some(activity_exhausted) {
        return Err("la preparación restaurada no fue equivalente".to_owned());
    }
    run_command(
        &mut restored,
        "deshabilitar ERICK para bloquear preparación",
        GameplayCommand::SetCompanionBehavior {
            companion: activity_erick.entity,
            behavior: gameplay::CompanionBehavior::Unavailable,
        },
    )?;
    if restored
        .world()
        .companion_preparation_assessment(activity_erick.entity)
        .map_err(|error| format!("consultar ERICK no disponible: {error:?}"))?
        .is_some()
    {
        return Err("ERICK no disponible conservó evaluación de preparación".to_owned());
    }
    println!(
        "PREPARACION DE ERICK COMPLETADA: combustible=5→60, capacidad_recompensa=true, enemigo={enemy}, reparacion=material_faltante, actividad_agotada=true, restaurado=true"
    );
    Ok(())
}

fn run_erick_coordination_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let origin = DestinationId::new(6_801);
    let destination = DestinationId::new(6_802);
    let mission = gameplay::types::MissionId(68);
    let objective = gameplay::types::ObjectiveId(6_801);
    for value in [
        Destination {
            id: origin,
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 10,
            discovery: DiscoveryState::Discovered,
        },
        Destination {
            id: destination,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
            travel_duration: Duration(1),
            fuel_cost: 10,
            discovery: DiscoveryState::Discovered,
        },
    ] {
        run_command(
            &mut session,
            "registrar destino de coordinacion",
            GameplayCommand::RegisterDestination { destination: value },
        )?;
    }
    run_command(
        &mut session,
        "registrar ruta de coordinacion",
        GameplayCommand::RegisterNavigationRoute {
            route: NavigationRoute {
                from: origin,
                to: destination,
                distance: 10.0,
                duration: Duration(1),
                fuel_cost: 10,
                state: NavigationRouteState::Available,
            },
        },
    )?;
    let derelict = DerelictId::new(6_801);
    run_command(
        &mut session,
        "registrar derelicto para pasos posteriores",
        GameplayCommand::RegisterDerelict {
            derelict: Derelict::new(derelict, Vec3::ZERO, 2.0).with_rewards(
                vec![DerelictReward {
                    item: ItemId(1),
                    amount: 1,
                }],
                RewardDistributionPolicy::Shared,
            ),
        },
    )?;
    run_command(
        &mut session,
        "descubrir derelicto para pasos posteriores",
        GameplayCommand::DiscoverDerelict { player, derelict },
    )?;
    let encounter = EncounterId::new(6_801);
    run_command(
        &mut session,
        "registrar encuentro para pasos posteriores",
        GameplayCommand::RegisterSpatialEncounter {
            encounter: SpatialEncounter::new(
                encounter,
                SpatialEncounterKind::Resource {
                    item: ItemId(1),
                    amount: 1,
                },
                Vec3::ZERO,
                2.0,
            ),
        },
    )?;
    run_command(
        &mut session,
        "activar encuentro para pasos posteriores",
        GameplayCommand::ActivateSpatialEncounter { encounter },
    )?;
    session
        .world_mut()
        .register_mission(
            gameplay::Mission::new(
                mission,
                "Coordinacion secuencial de ERICK",
                vec![gameplay::Objective::new(
                    objective,
                    mission,
                    "Alcanzar el siguiente objetivo",
                    true,
                    gameplay::ObjectiveTrigger::DestinationArrived { destination },
                )],
            )
            .map_err(|error| format!("definir mision de coordinacion: {error:?}"))?,
        )
        .map_err(|error| format!("registrar mision de coordinacion: {error:?}"))?;
    run_command(
        &mut session,
        "desbloquear mision de coordinacion",
        GameplayCommand::UnlockMission { mission },
    )?;
    run_command(
        &mut session,
        "iniciar mision de coordinacion",
        GameplayCommand::StartMission { mission },
    )?;
    run_command(
        &mut session,
        "crear ERICK para coordinar objetivos",
        GameplayCommand::CreateCompanion {
            followed_player: player,
            position: Vec3::ZERO,
        },
    )?;
    let erick = session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK no quedo registrado para coordinacion".to_owned())?;
    let ship = session.ids.first_ship;
    run_command(
        &mut session,
        "agotar combustible para bloquear preparacion",
        GameplayCommand::RemoveShipFuel {
            ship,
            amount: 60,
        },
    )?;
    let blocked_preparation = session
        .world()
        .companion_coordination_assessment(erick.entity)
        .map_err(|error| format!("evaluar preparacion bloqueante: {error:?}"))?
        .ok_or_else(|| "no hubo evaluacion de preparacion bloqueante".to_owned())?;
    if !blocked_preparation.blocked
        || blocked_preparation.next_step.as_ref().is_none_or(|step| {
            step.category != gameplay::CoordinationCategory::Preparation
                || step.priority != gameplay::CoordinationPriority::Critical
        })
    {
        return Err(format!("preparacion no bloqueo la secuencia: {blocked_preparation:?}"));
    }
    run_command(
        &mut session,
        "resolver preparacion de combustible",
        GameplayCommand::AddShipFuel {
            ship,
            amount: 60,
        },
    )?;
    let initial = session
        .world()
        .companion_coordination_assessment(erick.entity)
        .map_err(|error| format!("evaluar coordinacion inicial: {error:?}"))?
        .ok_or_else(|| "no hubo evaluacion inicial de coordinacion".to_owned())?;
    let initial_step = initial
        .next_step
        .as_ref()
        .ok_or_else(|| "ERICK no propuso un siguiente paso".to_owned())?;
    if initial.objective != Some(objective)
        || initial.mission != Some(mission)
        || initial.blocked
        || initial_step.category != gameplay::CoordinationCategory::Navigation
        || initial_step.action != gameplay::CoordinationAction::StartAutopilot(destination)
        || initial_step.order != 1
        || initial.later_steps.len() < 2
        || initial.later_steps[0].order != 2
        || initial.later_steps[1].order != 3
    {
        return Err(format!("coordinacion inicial inesperada: {initial:?}"));
    }
    let snapshot = session.save_state();
    let repeated = session
        .world()
        .companion_coordination_assessment(erick.entity)
        .map_err(|error| format!("repetir coordinacion inicial: {error:?}"))?;
    if repeated != Some(initial.clone()) || session.save_state().serialize() != snapshot.serialize() {
        return Err("la coordinacion inicial no fue determinista o modifico el mundo".to_owned());
    }
    run_command(
        &mut session,
        "crear amenaza para coordinacion tactica",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Shooter, Vec3::ZERO),
        },
    )?;
    let enemy = session
        .world()
        .enemies()
        .next()
        .map(|value| value.entity)
        .ok_or_else(|| "la amenaza de coordinacion no quedo registrada".to_owned())?;
    run_command(
        &mut session,
        "equipar arma para resolver riesgo",
        GameplayCommand::EquipPlayerWeapon {
            player,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                15,
                10,
                Duration(1_000),
                WeaponAmmunition {
                    item: ItemId(2),
                    amount_per_attack: 1,
                },
            ),
        },
    )?;
    run_command(
        &mut session,
        "preparar municion para resolver riesgo",
        GameplayCommand::AddInventoryItem {
            player,
            item: ItemId(2),
            amount: 1,
        },
    )?;
    let risk = session
        .world()
        .companion_coordination_assessment(erick.entity)
        .map_err(|error| format!("evaluar riesgo tactico: {error:?}"))?
        .ok_or_else(|| "no hubo evaluacion de riesgo tactico".to_owned())?;
    let risk_step = risk
        .next_step
        .as_ref()
        .ok_or_else(|| "no hubo paso tactico inmediato".to_owned())?;
    if risk_step.category != gameplay::CoordinationCategory::Combat
        || risk_step.action != gameplay::CoordinationAction::Tactical(gameplay::TacticalAction::DefeatEnemy(enemy))
        || risk_step.priority != gameplay::CoordinationPriority::Critical
    {
        return Err(format!("coordinacion tactica inesperada: {risk:?}"));
    }
    run_command(
        &mut session,
        "retirar amenaza para continuar coordinacion",
        GameplayCommand::SetEnemyActive {
            enemy,
            active: false,
        },
    )?;
    let after_risk = session
        .world()
        .companion_coordination_assessment(erick.entity)
        .map_err(|error| format!("reevaluar coordinacion: {error:?}"))?
        .ok_or_else(|| "no hubo reevaluacion de coordinacion".to_owned())?;
    if after_risk.next_step.is_none()
        || after_risk.next_step.as_ref().is_some_and(|step| {
            step.category != gameplay::CoordinationCategory::Navigation
                || step.action != gameplay::CoordinationAction::StartAutopilot(destination)
        })
    {
        return Err(format!("coordinacion no volvio a la ruta: {after_risk:?}"));
    }
    let station = session
        .create_station(Vec3::ZERO, 2.0, StationType::Piloting)
        .map_err(|error| format!("crear estacion de coordinacion: {error:?}"))?;
    run_command(
        &mut session,
        "ocupar estacion para completar objetivo",
        GameplayCommand::Interact {
            player,
            target: station,
        },
    )?;
    run_command(
        &mut session,
        "planificar ruta de coordinacion",
        GameplayCommand::PlanAutopilot {
            player,
            pilot_station: station,
            origin,
            destination,
        },
    )?;
    let planned = session
        .world()
        .companion_coordination_assessment(erick.entity)
        .map_err(|error| format!("evaluar autopilot planificado: {error:?}"))?
        .ok_or_else(|| "no hubo evaluacion con autopilot planificado".to_owned())?;
    if planned.next_step.as_ref().is_none_or(|step| {
        step.action != gameplay::CoordinationAction::MonitorAutopilot(destination)
    }) {
        return Err(format!("autopilot planificado no se monitorizo: {planned:?}"));
    }
    run_command(
        &mut session,
        "iniciar ruta de coordinacion",
        GameplayCommand::StartAutopilot {
            player,
            pilot_station: station,
        },
    )?;
    run_command(
        &mut session,
        "avanzar tiempo para completar objetivo",
        GameplayCommand::AdvanceTime { delta: Duration(1) },
    )?;
    let completed = session
        .mission_progress(mission)
        .ok_or_else(|| "no hubo progreso de mision tras la llegada".to_owned())?;
    if completed.state != MissionState::Completed || completed.required_completed != 1 {
        return Err(format!("objetivo no completado por la ruta: {completed:?}"));
    }
    let after_completion = session
        .world()
        .companion_coordination_assessment(erick.entity)
        .map_err(|error| format!("evaluar coordinacion tras completar objetivo: {error:?}"))?
        .ok_or_else(|| "no hubo evaluacion tras completar objetivo".to_owned())?;
    if after_completion.objective == Some(objective)
        || after_completion.next_step.as_ref().is_some_and(|step| step.objective == Some(objective))
    {
        return Err(format!("ERICK mantuvo objetivo completado: {after_completion:?}"));
    }
    let completion_explanation = session
        .world()
        .companion_recommendation_explanation(erick.entity)
        .map_err(|error| format!("explicar objetivo completado: {error:?}"))?
        .ok_or_else(|| "no hubo explicacion tras completar objetivo".to_owned())?;
    if completion_explanation.objective == Some(objective)
        || completion_explanation.status == gameplay::ExplanationStatus::Blocked
    {
        return Err(format!("explicacion mantuvo objetivo completado: {completion_explanation:?}"));
    }
    let completion_consistency = session
        .world()
        .companion_consistency_assessment(erick.entity)
        .map_err(|error| format!("auditar objetivo completado: {error:?}"))?
        .ok_or_else(|| "no hubo auditoria tras completar objetivo".to_owned())?;
    if completion_consistency.state == gameplay::ConsistencyAuditState::Contradictory
        || completion_consistency.contradictions.iter().any(|value| {
            value.reason == gameplay::ContradictionReason::CompletedObjectiveStillRecommended
        })
    {
        return Err(format!("auditoria mantuvo objetivo completado: {completion_consistency:?}"));
    }
    run_command(
        &mut session,
        "marcar ERICK no disponible",
        GameplayCommand::SetCompanionBehavior {
            companion: erick.entity,
            behavior: gameplay::CompanionBehavior::Unavailable,
        },
    )?;
    if session
        .world()
        .companion_coordination_assessment(erick.entity)
        .map_err(|error| format!("consultar ERICK no disponible: {error:?}"))?
        .is_some()
    {
        return Err("ERICK no disponible conservo coordinacion".to_owned());
    }
    run_command(
        &mut session,
        "reactivar ERICK para evaluar destruccion",
        GameplayCommand::SetCompanionBehavior {
            companion: erick.entity,
            behavior: gameplay::CompanionBehavior::Accompany,
        },
    )?;
    run_command(
        &mut session,
        "destruir nave para bloquear coordinacion",
        GameplayCommand::DamageShip {
            ship,
            amount: 100,
        },
    )?;
    let destroyed = session
        .world()
        .companion_coordination_assessment(erick.entity)
        .map_err(|error| format!("evaluar nave destruida: {error:?}"))?
        .ok_or_else(|| "no hubo evaluacion con nave destruida".to_owned())?;
    if !destroyed.blocked
        || destroyed.next_step.as_ref().is_none_or(|step| {
            step.priority != gameplay::CoordinationPriority::Critical
                || step.status != gameplay::CoordinationStepStatus::Blocked
        })
    {
        return Err(format!("nave destruida no bloqueo coordinacion: {destroyed:?}"));
    }
    println!(
        "COORDINACION SECUENCIAL DE ERICK COMPLETADA: mision={mission:?}, objetivo={objective:?}, ruta={destination:?}, riesgo={enemy:?}, pasos_futuros={}",
        after_risk.later_steps.len()
    );
    Ok(())
}

fn run_erick_recovery_scenario() -> Result<(), String> {
    let mut session = ProjectEchoSession::new();
    let player = session.ids.first_player;
    let ship = session.ids.first_ship;
    let origin = DestinationId::new(6_901);
    let destination = DestinationId::new(6_902);
    let mission = gameplay::types::MissionId(69);
    let objective = gameplay::types::ObjectiveId(6_901);
    for value in [
        Destination {
            id: origin,
            destination_type: DestinationType::Planet,
            position: Vec3::ZERO,
            travel_duration: Duration(1),
            fuel_cost: 10,
            discovery: DiscoveryState::Discovered,
        },
        Destination {
            id: destination,
            destination_type: DestinationType::Planet,
            position: Vec3 { x: 10.0, y: 0.0, z: 0.0 },
            travel_duration: Duration(1),
            fuel_cost: 10,
            discovery: DiscoveryState::Discovered,
        },
    ] {
        run_command(
            &mut session,
            "registrar destino de recuperacion",
            GameplayCommand::RegisterDestination { destination: value },
        )?;
    }
    run_command(
        &mut session,
        "registrar ruta de recuperacion",
        GameplayCommand::RegisterNavigationRoute {
            route: NavigationRoute {
                from: origin,
                to: destination,
                distance: 10.0,
                duration: Duration(1),
                fuel_cost: 10,
                state: NavigationRouteState::Available,
            },
        },
    )?;
    session
        .world_mut()
        .register_mission(
            gameplay::Mission::new(
                mission,
                "Diagnostico de recuperacion de ERICK",
                vec![gameplay::Objective::new(
                    objective,
                    mission,
                    "Recuperar la ruta",
                    true,
                    gameplay::ObjectiveTrigger::DestinationArrived { destination },
                )],
            )
            .map_err(|error| format!("definir mision de recuperacion: {error:?}"))?,
        )
        .map_err(|error| format!("registrar mision de recuperacion: {error:?}"))?;
    run_command(
        &mut session,
        "desbloquear mision de recuperacion",
        GameplayCommand::UnlockMission { mission },
    )?;
    run_command(
        &mut session,
        "iniciar mision de recuperacion",
        GameplayCommand::StartMission { mission },
    )?;
    run_command(
        &mut session,
        "crear ERICK para diagnostico",
        GameplayCommand::CreateCompanion {
            followed_player: player,
            position: Vec3::ZERO,
        },
    )?;
    let erick = session
        .world()
        .companions()
        .next()
        .copied()
        .ok_or_else(|| "ERICK no quedo registrado para diagnostico".to_owned())?;
    run_command(
        &mut session,
        "agotar combustible para diagnosticar bloqueo",
        GameplayCommand::RemoveShipFuel { ship, amount: 60 },
    )?;
    let fuel_block = session
        .world()
        .companion_recovery_assessment(erick.entity)
        .map_err(|error| format!("diagnosticar combustible: {error:?}"))?
        .ok_or_else(|| "no hubo diagnostico de combustible".to_owned())?;
    if fuel_block.status != gameplay::RecoveryStatus::Blocked
        || fuel_block.blocker.as_ref().map(|value| value.reason)
            != Some(gameplay::RecoveryReason::FuelInsufficient)
        || !fuel_block.alternatives.is_empty()
        || fuel_block.actionable
    {
        return Err(format!("diagnostico de combustible inesperado: {fuel_block:?}"));
    }
    let fuel_explanation = session
        .world()
        .companion_recommendation_explanation(erick.entity)
        .map_err(|error| format!("explicar bloqueo de combustible: {error:?}"))?
        .ok_or_else(|| "no hubo explicacion de combustible".to_owned())?;
    if fuel_explanation.status != gameplay::ExplanationStatus::Blocked
        || fuel_explanation.reason != gameplay::ExplanationReason::FuelInsufficient
        || !fuel_explanation
            .evidence
            .iter()
            .any(|value| value.kind == gameplay::ExplanationEvidenceKind::Fuel && !value.satisfied)
    {
        return Err(format!("explicacion de combustible inesperada: {fuel_explanation:?}"));
    }
    let fuel_consistency = session
        .world()
        .companion_consistency_assessment(erick.entity)
        .map_err(|error| format!("auditar consistencia de combustible: {error:?}"))?
        .ok_or_else(|| "no hubo auditoria de consistencia de combustible".to_owned())?;
    if fuel_consistency.state != gameplay::ConsistencyAuditState::Consistent
        || !fuel_consistency.contradictions.is_empty()
    {
        return Err(format!("consistencia de combustible inesperada: {fuel_consistency:?}"));
    }
    let consistency_snapshot = session.save_state().serialize();
    let repeated_consistency = session
        .world()
        .companion_consistency_assessment(erick.entity)
        .map_err(|error| format!("repetir auditoria de combustible: {error:?}"))?;
    if repeated_consistency != Some(fuel_consistency.clone())
        || session.save_state().serialize() != consistency_snapshot
    {
        return Err("la auditoria de consistencia no fue determinista o mutó el mundo".to_owned());
    }
    let saved_block = session.save_state();
    let repeated_block = session
        .world()
        .companion_recovery_assessment(erick.entity)
        .map_err(|error| format!("repetir diagnostico de combustible: {error:?}"))?;
    if repeated_block != Some(fuel_block.clone())
        || session.save_state().serialize() != saved_block.serialize()
    {
        return Err("el diagnostico de combustible no fue determinista o mutó el mundo".to_owned());
    }
    run_command(
        &mut session,
        "resolver combustible mediante API existente",
        GameplayCommand::AddShipFuel { ship, amount: 60 },
    )?;
    let route_ready = session
        .world()
        .companion_recovery_assessment(erick.entity)
        .map_err(|error| format!("diagnosticar ruta recuperada: {error:?}"))?
        .ok_or_else(|| "no hubo diagnostico de ruta recuperada".to_owned())?;
    if route_ready.status != gameplay::RecoveryStatus::Actionable
        || route_ready.blocker.is_some()
        || route_ready.attempted.as_ref().is_none_or(|step| {
            step.action != gameplay::CoordinationAction::StartAutopilot(destination)
        })
    {
        return Err(format!("ruta recuperada inesperada: {route_ready:?}"));
    }
    let route_explanation = session
        .world()
        .companion_recommendation_explanation(erick.entity)
        .map_err(|error| format!("explicar recomendacion de ruta: {error:?}"))?
        .ok_or_else(|| "no hubo explicacion de ruta".to_owned())?;
    if route_explanation.status != gameplay::ExplanationStatus::Actionable
        || route_explanation.reason != gameplay::ExplanationReason::ObjectiveAction
        || !route_explanation
            .evidence
            .iter()
            .any(|value| value.kind == gameplay::ExplanationEvidenceKind::Route && value.satisfied)
    {
        return Err(format!("explicacion de ruta inesperada: {route_explanation:?}"));
    }
    let route_consistency = session
        .world()
        .companion_consistency_assessment(erick.entity)
        .map_err(|error| format!("auditar consistencia de ruta: {error:?}"))?
        .ok_or_else(|| "no hubo auditoria de consistencia de ruta".to_owned())?;
    if route_consistency.state == gameplay::ConsistencyAuditState::Contradictory
        || !route_consistency.contradictions.is_empty()
    {
        return Err(format!("consistencia de ruta inesperada: {route_consistency:?}"));
    }
    let route_change = gameplay::RecommendationExplanation::compare(
        &fuel_explanation,
        &route_explanation,
    );
    if route_change.reason != gameplay::ExplanationChangeReason::PreparationResolved {
        return Err(format!("comparacion de preparacion inesperada: {route_change:?}"));
    }
    run_command(
        &mut session,
        "crear amenaza para diagnostico de combate",
        GameplayCommand::CreateEnemy {
            configuration: EnemyConfiguration::for_archetype(EnemyArchetype::Charger, Vec3::ZERO),
        },
    )?;
    let enemy = session
        .world()
        .enemies()
        .next()
        .map(|value| value.entity)
        .ok_or_else(|| "amenaza no registrada para diagnostico".to_owned())?;
    run_command(
        &mut session,
        "equipar arma para diagnostico de combate",
        GameplayCommand::EquipPlayerWeapon {
            player,
            weapon: WeaponConfiguration::with_ammunition(
                AttackType::Kinetic,
                15,
                10,
                Duration(1_000),
                WeaponAmmunition {
                    item: ItemId(2),
                    amount_per_attack: 1,
                },
            ),
        },
    )?;
    run_command(
        &mut session,
        "añadir municion para diagnostico de combate",
        GameplayCommand::AddInventoryItem {
            player,
            item: ItemId(2),
            amount: 2,
        },
    )?;
    let threat = session
        .world()
        .companion_recovery_assessment(erick.entity)
        .map_err(|error| format!("diagnosticar amenaza: {error:?}"))?
        .ok_or_else(|| "no hubo diagnostico de amenaza".to_owned())?;
    if threat.blocker.as_ref().map(|value| value.reason)
            != Some(gameplay::RecoveryReason::ActiveThreat)
        || threat.blocker.as_ref().and_then(|value| value.entity) != Some(enemy)
        || threat.status != gameplay::RecoveryStatus::Recoverable
        || !threat.actionable
    {
        return Err(format!("diagnostico de amenaza inesperado: {threat:?}"));
    }
    let threat_explanation = session
        .world()
        .companion_recommendation_explanation(erick.entity)
        .map_err(|error| format!("explicar amenaza activa: {error:?}"))?
        .ok_or_else(|| "no hubo explicacion de amenaza".to_owned())?;
    if threat_explanation.reason != gameplay::ExplanationReason::ActiveThreat
        || threat_explanation.status != gameplay::ExplanationStatus::Recoverable
        || !threat_explanation
            .evidence
            .iter()
            .any(|value| value.kind == gameplay::ExplanationEvidenceKind::Threat && !value.satisfied)
    {
        return Err(format!("explicacion de amenaza inesperada: {threat_explanation:?}"));
    }
    let threat_consistency = session
        .world()
        .companion_consistency_assessment(erick.entity)
        .map_err(|error| format!("auditar consistencia de amenaza: {error:?}"))?
        .ok_or_else(|| "no hubo auditoria de consistencia de amenaza".to_owned())?;
    if threat_consistency.state == gameplay::ConsistencyAuditState::Contradictory
        || !threat_consistency.contradictions.is_empty()
    {
        return Err(format!("consistencia de amenaza inesperada: {threat_consistency:?}"));
    }
    run_command(
        &mut session,
        "derrotar amenaza para actualizar diagnostico",
        GameplayCommand::AttackEnemy { player, enemy },
    )?;
    run_command(
        &mut session,
        "esperar enfriamiento para derrotar amenaza",
        GameplayCommand::AdvanceTime {
            delta: Duration(1_000),
        },
    )?;
    run_command(
        &mut session,
        "completar derrota para actualizar diagnostico",
        GameplayCommand::AttackEnemy { player, enemy },
    )?;
    let after_enemy = session
        .world()
        .companion_recovery_assessment(erick.entity)
        .map_err(|error| format!("diagnosticar despues de la amenaza: {error:?}"))?
        .ok_or_else(|| "no hubo diagnostico despues de la amenaza".to_owned())?;
    if after_enemy.blocker.as_ref().is_some_and(|value| value.entity == Some(enemy)) {
        return Err("la amenaza derrotada siguio bloqueando la recuperacion".to_owned());
    }
    let after_enemy_explanation = session
        .world()
        .companion_recommendation_explanation(erick.entity)
        .map_err(|error| format!("explicar estado tras amenaza: {error:?}"))?
        .ok_or_else(|| "no hubo explicacion tras amenaza".to_owned())?;
    let threat_change = gameplay::RecommendationExplanation::compare(
        &threat_explanation,
        &after_enemy_explanation,
    );
    if threat_change.reason != gameplay::ExplanationChangeReason::ThreatResolved {
        return Err(format!("comparacion de amenaza inesperada: {threat_change:?}"));
    }
    let after_enemy_consistency = session
        .world()
        .companion_consistency_assessment(erick.entity)
        .map_err(|error| format!("auditar consistencia tras amenaza: {error:?}"))?
        .ok_or_else(|| "no hubo auditoria tras amenaza".to_owned())?;
    if after_enemy_consistency.state == gameplay::ConsistencyAuditState::Contradictory
        || !after_enemy_consistency.contradictions.is_empty()
    {
        return Err(format!("consistencia tras amenaza inesperada: {after_enemy_consistency:?}"));
    }
    let saved_ready = session.save_state();
    let mut restored = ProjectEchoSession::new();
    restored
        .restore_state(saved_ready)
        .map_err(|error| format!("restaurar diagnostico de recuperacion: {error}"))?;
    let restored_assessment = restored
        .world()
        .companion_recovery_assessment(erick.entity)
        .map_err(|error| format!("consultar diagnostico restaurado: {error:?}"))?;
    if restored_assessment != Some(after_enemy.clone()) {
        return Err("el diagnostico restaurado no fue equivalente".to_owned());
    }
    let restored_explanation = restored
        .world()
        .companion_recommendation_explanation(erick.entity)
        .map_err(|error| format!("explicar estado restaurado: {error:?}"))?;
    if restored_explanation != Some(after_enemy_explanation.clone()) {
        return Err("la explicacion restaurada no fue equivalente".to_owned());
    }
    let restored_consistency = restored
        .world()
        .companion_consistency_assessment(erick.entity)
        .map_err(|error| format!("auditar consistencia restaurada: {error:?}"))?;
    if restored_consistency != Some(after_enemy_consistency.clone()) {
        return Err("la consistencia restaurada no fue equivalente".to_owned());
    }
    run_command(
        &mut restored,
        "desactivar jugador para diagnostico bloqueado",
        GameplayCommand::SetPlayerActive {
            player,
            active: false,
        },
    )?;
    if restored
        .world()
        .companion_recovery_assessment(erick.entity)
        .map_err(|error| format!("consultar jugador inactivo: {error:?}"))?
        .is_some()
    {
        return Err("el jugador inactivo conservo diagnostico accionable".to_owned());
    }
    if restored
        .world()
        .companion_recommendation_explanation(erick.entity)
        .map_err(|error| format!("consultar explicacion con jugador inactivo: {error:?}"))?
        .is_some()
    {
        return Err("el jugador inactivo conservo explicacion accionable".to_owned());
    }
    if restored
        .world()
        .companion_consistency_assessment(erick.entity)
        .map_err(|error| format!("consultar consistencia con jugador inactivo: {error:?}"))?
        .is_some()
    {
        return Err("el jugador inactivo conservo auditoria accionable".to_owned());
    }
    println!(
        "DIAGNOSTICO DE RECUPERACION DE ERICK COMPLETADO: combustible={:?}, ruta={:?}, amenaza={:?}, resuelta={:?}, no_reproducibles=bloqueo-explicacion/ruta-inviable, no_evaluable=disponibilidad, restaurado=true",
        fuel_consistency.state,
        route_consistency.state,
        threat_consistency.state,
        after_enemy_consistency.state
    );
    Ok(())
}

fn run_scenario() -> Result<(), String> {
    run_atomicity_regressions()?;
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
    run_integrated_scenario()?;
    run_anomaly_mission_scenario()?;
    run_gravitational_storm_scenario()?;
    run_shielded_enemy_scenario()?;
    run_cooperative_revival_scenario()?;
    run_cooperative_combat_scenario()?;
    run_cooperative_progression_scenario()?;
    run_player_count_scenario(1)?;
    run_player_count_scenario(2)?;
    run_player_count_scenario(3)?;
    run_inventory_transfer_scenario()?;
    run_reward_distribution_scenario()?;
    run_autonomous_enemy_scenario()?;
    run_spatial_encounter_scenario()?;
    run_autopilot_scenario()?;
    run_derelict_scenario()?;
    run_functional_world_scenario()?;
    run_guidance_progression_scenario()?;
    run_group_scaling_scenario()?;
    run_persistence_scenario()?;
    run_erick_companion_scenario()?;
    run_erick_interaction_scenario()?;
    run_erick_tactical_scenario()?;
    run_erick_cooperative_tactical_scenario()?;
    run_erick_cooperative_support_scenario()?;
    run_erick_cooperative_readiness_scenario()?;
    run_erick_navigation_scenario()?;
    run_erick_route_risk_scenario()?;
    run_erick_route_comparison_scenario()?;
    run_erick_operational_scenario()?;
    run_erick_exploration_coordination_scenario()?;
    run_erick_preparation_scenario()?;
    run_erick_coordination_scenario()?;
    run_erick_recovery_scenario()?;
    Ok(())
}