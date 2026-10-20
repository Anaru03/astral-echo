use gameplay::types::{MissionId, ObjectiveId};
use gameplay::{EnemyArchetype, Mission, MissionDefinitionError, Objective, ObjectiveTrigger};

pub const ECOS_DE_LA_ANOMALIA: MissionId = MissionId(2);
pub const CONFIRMAR_SENAL: ObjectiveId = ObjectiveId(7);
pub const INVESTIGAR_DESTINO: ObjectiveId = ObjectiveId(8);
pub const NEUTRALIZAR_AMENAZA: ObjectiveId = ObjectiveId(9);
pub const RECUPERAR_INFORMACION: ObjectiveId = ObjectiveId(10);

pub const SIGNAL_DESTINATION: gameplay::DestinationId = gameplay::DestinationId::new(102);
pub const PLANET_DESTINATION: gameplay::DestinationId = gameplay::DestinationId::new(101);

pub fn definition() -> Result<Mission, MissionDefinitionError> {
    let confirm_signal = Objective::new(
        CONFIRMAR_SENAL,
        ECOS_DE_LA_ANOMALIA,
        "Confirmar la señal desconocida",
        true,
        ObjectiveTrigger::DestinationIdentified {
            destination: Some(SIGNAL_DESTINATION),
        },
    );
    let investigate_destination = Objective::new(
        INVESTIGAR_DESTINO,
        ECOS_DE_LA_ANOMALIA,
        "Investigar el destino",
        true,
        ObjectiveTrigger::PlanetDiscovered {
            destination: PLANET_DESTINATION,
        },
    )
    .with_dependencies([CONFIRMAR_SENAL]);
    let neutralize_threat = Objective::new(
        NEUTRALIZAR_AMENAZA,
        ECOS_DE_LA_ANOMALIA,
        "Neutralizar la amenaza",
        true,
        ObjectiveTrigger::EnemiesDefeated {
            archetype: Some(EnemyArchetype::Shooter),
            count: 1,
        },
    )
    .with_dependencies([INVESTIGAR_DESTINO]);
    let recover_information = Objective::new(
        RECUPERAR_INFORMACION,
        ECOS_DE_LA_ANOMALIA,
        "Recuperar información util",
        true,
        ObjectiveTrigger::ItemsCollected {
            item: gameplay::types::ItemId(3),
            amount: 1,
        },
    )
    .with_dependencies([NEUTRALIZAR_AMENAZA]);

    Mission::new(
        ECOS_DE_LA_ANOMALIA,
        "Ecos de la anomalía",
        vec![
            confirm_signal,
            investigate_destination,
            neutralize_threat,
            recover_information,
        ],
    )
    .map(|mission| {
        mission.with_description(
            "ECHO-7 sigue una señal identificada para investigar una anomalía y recuperar información.",
        )
    })
}
