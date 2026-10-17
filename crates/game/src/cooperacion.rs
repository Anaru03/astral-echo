use gameplay::types::{MissionId, ObjectiveId};
use gameplay::{EnemyArchetype, Mission, MissionDefinitionError, Objective, ObjectiveTrigger};

pub const PROGRESION_COOPERATIVA: MissionId = MissionId(5);
pub const IDENTIFICAR_SENAL: ObjectiveId = ObjectiveId(20);
pub const DERROTAR_AMENAZA: ObjectiveId = ObjectiveId(21);
pub const RECUPERAR_RECOMPENSA: ObjectiveId = ObjectiveId(22);
pub const SIGNAL_DESTINATION: gameplay::DestinationId = gameplay::DestinationId::new(104);

pub fn definition() -> Result<Mission, MissionDefinitionError> {
    let identify_signal = Objective::new(
        IDENTIFICAR_SENAL,
        PROGRESION_COOPERATIVA,
        "Identificar la señal en equipo",
        true,
        ObjectiveTrigger::DestinationIdentified {
            destination: Some(SIGNAL_DESTINATION),
        },
    );
    let defeat_enemy = Objective::new(
        DERROTAR_AMENAZA,
        PROGRESION_COOPERATIVA,
        "Neutralizar la amenaza en equipo",
        true,
        ObjectiveTrigger::EnemiesDefeated {
            archetype: Some(EnemyArchetype::Shooter),
            count: 1,
        },
    )
    .with_dependencies([IDENTIFICAR_SENAL]);
    let recover_reward = Objective::new(
        RECUPERAR_RECOMPENSA,
        PROGRESION_COOPERATIVA,
        "Recuperar la recompensa",
        true,
        ObjectiveTrigger::ItemsCollected {
            item: gameplay::types::ItemId(3),
            amount: 1,
        },
    )
    .with_dependencies([DERROTAR_AMENAZA]);

    Mission::new(
        PROGRESION_COOPERATIVA,
        "Progresión cooperativa",
        vec![identify_signal, defeat_enemy, recover_reward],
    )
    .map(|mission| {
        mission.with_description(
            "Dos tripulantes coordinan la identificación, la defensa y la recuperación de una señal.",
        )
    })
}