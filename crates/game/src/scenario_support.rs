use super::*;

pub(crate) fn run_command(
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

pub(crate) fn expect_transfer_rejection(
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
        .update([GameplayCommand::TransferInventoryItem { from, to, item, amount }])
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
        return Err(format!("{label}: el rechazó dejó cambios parciales"));
    }
    Ok(())
}

pub(crate) fn required_completed(session: &ProjectEchoSession) -> Result<MissionProgress, String> {
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