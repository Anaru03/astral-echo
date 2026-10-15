pub(super) fn run() -> Result<(), String> {
    super::super::run_ship_upgrade_scenario()?;
    super::super::run_resource_mission_integration_scenario()?;
    super::super::run_integrated_expedition_scenario()?;
    super::super::run_full_expedition_regression_scenario()?;
    super::super::run_guidance_progression_scenario()?;
    super::super::run_group_scaling_scenario()?;
    super::super::run_cooperative_scaling_expedition_scenario()?;
    super::super::run_survival_recovery_expedition_scenario()?;
    super::super::run_persistence_scenario()?;
    Ok(())
}