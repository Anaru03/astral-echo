use gameplay::types::{MissionId, ObjectiveId};
use gameplay::{DestinationId, Mission, MissionDefinitionError, Objective, ObjectiveTrigger};

pub const TORMENTA_GRAVITACIONAL: MissionId = MissionId(3);
pub const ENTRAR_EN_LA_TORMENTA: ObjectiveId = ObjectiveId(11);
pub const ESTABILIZAR_LA_NAVE: ObjectiveId = ObjectiveId(12);
pub const CONTINUAR_EL_VIAJE: ObjectiveId = ObjectiveId(13);

pub const HAZARD: gameplay::HazardId = gameplay::HazardId::new(301);
pub const ESCAPE_DESTINATION: DestinationId = DestinationId::new(103);

pub fn definition() -> Result<Mission, MissionDefinitionError> {
    let enter_storm = Objective::new(
        ENTRAR_EN_LA_TORMENTA,
        TORMENTA_GRAVITACIONAL,
        "Entrar en la tormenta gravitacional",
        true,
        ObjectiveTrigger::ShipDamagedByHazard {
            hazard: Some(HAZARD),
        },
    );
    let stabilize_ship = Objective::new(
        ESTABILIZAR_LA_NAVE,
        TORMENTA_GRAVITACIONAL,
        "Estabilizar la nave",
        true,
        ObjectiveTrigger::ShipRepaired { ship: None },
    )
    .with_dependencies([ENTRAR_EN_LA_TORMENTA]);
    let continue_travel = Objective::new(
        CONTINUAR_EL_VIAJE,
        TORMENTA_GRAVITACIONAL,
        "Continuar el viaje",
        true,
        ObjectiveTrigger::DestinationArrived {
            destination: ESCAPE_DESTINATION,
        },
    )
    .with_dependencies([ESTABILIZAR_LA_NAVE]);

    Mission::new(
        TORMENTA_GRAVITACIONAL,
        "Tormenta gravitacional",
        vec![enter_storm, stabilize_ship, continue_travel],
    )
    .map(|mission| {
        mission.with_description(
            "Una perturbacion gravitacional dana ECHO-7 y obliga a estabilizarla antes de continuar.",
        )
    })
}