pub(crate) fn run_navigation_and_exploration() -> Result<(), String> {
    super::run_spatial_encounter_scenario()?;
    super::run_dynamic_spatial_event_scenario()?;
    super::run_spatial_encounter_consequence_scenario()?;
    super::run_autopilot_scenario()?;
    super::run_autopilot_interruption_recovery_scenario()?;
    super::run_autopilot_spatial_encounter_scenario()?;
    super::run_autopilot_spatial_encounter_interrupt_scenario()?;
    super::run_autopilot_chained_encounters_scenario()?;
    super::run_derelict_scenario()?;
    super::run_derelict_exploration_flow_scenario()?;
    super::run_exploration_continuity_scenario()?;
    super::run_functional_world_scenario()?;
    Ok(())
}

pub(crate) fn run_combat_and_expeditions() -> Result<(), String> {
    super::run_integrated_scenario()?;
    super::run_anomaly_mission_scenario()?;
    super::run_end_to_end_coherence_scenario()?;
    super::run_gravitational_storm_scenario()?;
    super::run_shielded_enemy_scenario()?;
    super::run_cooperative_revival_scenario()?;
    super::run_cooperative_combat_scenario()?;
    super::run_cooperative_progression_scenario()?;
    super::run_player_count_scenario(1)?;
    super::run_player_count_scenario(2)?;
    super::run_player_count_scenario(3)?;
    super::run_inventory_transfer_scenario()?;
    super::run_reward_distribution_scenario()?;
    super::run_autonomous_enemy_scenario()?;
    super::run_integrated_tactical_combat_scenario()?;
    super::run_integrated_cooperative_combat_recovery_scenario()?;
    super::run_multi_enemy_cooperative_combat_scenario()?;
    Ok(())
}

pub(crate) fn run_progression_and_systems() -> Result<(), String> {
    super::run_ship_upgrade_scenario()?;
    super::run_resource_mission_integration_scenario()?;
    super::run_integrated_expedition_scenario()?;
    super::run_full_expedition_regression_scenario()?;
    super::run_guidance_progression_scenario()?;
    super::run_group_scaling_scenario()?;
    super::run_cooperative_scaling_expedition_scenario()?;
    super::run_survival_recovery_expedition_scenario()?;
    super::run_persistence_scenario()?;
    Ok(())
}

pub(crate) fn run_erick() -> Result<(), String> {
    super::run_erick_companion_scenario()?;
    super::run_erick_interaction_scenario()?;
    super::run_erick_tactical_scenario()?;
    super::run_erick_cooperative_tactical_scenario()?;
    super::run_erick_cooperative_support_scenario()?;
    super::run_erick_cooperative_readiness_scenario()?;
    super::run_erick_cooperative_navigation_risk_scenario()?;
    super::run_erick_integrated_recovery_scenario()?;
    super::run_erick_task_compatibility_scenario()?;
    super::run_erick_navigation_contingency_scenario()?;
    super::run_erick_cooperative_recovery_assignment_scenario()?;
    super::run_erick_navigation_scenario()?;
    super::run_erick_route_risk_scenario()?;
    super::run_erick_route_comparison_scenario()?;
    super::run_erick_operational_scenario()?;
    super::run_erick_exploration_coordination_scenario()?;
    super::run_erick_preparation_scenario()?;
    super::run_erick_coordination_scenario()?;
    super::run_erick_recovery_scenario()?;
    Ok(())
}