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
    CommandStatus, GameplayCommand, GameplayEvent, LifeSupportConfiguration,
    LifeSupportRegistrationError, MissionProgress, OxygenCompartmentConfiguration,
    RewardDistributionPolicy, ShipRepairConfiguration, SignalKnowledge, StationState,
    StationType, TravelState, WeaponAmmunition, WeaponConfiguration,
};

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

fn main() {
    if let Err(error) = run_scenario() {
        eprintln!("ESCENARIO FALLIDO: {error}");
        std::process::exit(1);
    }
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
    Ok(())
}