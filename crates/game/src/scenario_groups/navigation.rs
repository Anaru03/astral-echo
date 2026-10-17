pub(super) fn run() -> Result<(), String> {
    super::super::run_spatial_encounter_scenario()?;
    super::super::run_dynamic_spatial_event_scenario()?;
    super::super::run_spatial_encounter_consequence_scenario()?;
    super::super::run_autopilot_scenario()?;
    super::super::run_autopilot_interruption_recovery_scenario()?;
    super::super::run_autopilot_spatial_encounter_scenario()?;
    super::super::run_autopilot_spatial_encounter_interrupt_scenario()?;
    super::super::run_autopilot_chained_encounters_scenario()?;
    super::super::run_derelict_scenario()?;
    super::super::run_derelict_exploration_flow_scenario()?;
    super::super::run_exploration_continuity_scenario()?;
    super::super::run_functional_world_scenario()?;
    Ok(())
}