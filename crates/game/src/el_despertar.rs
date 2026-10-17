use gameplay::types::{MissionId, ObjectiveId};
use gameplay::{
    ConsoleRole, DoorRole, Mission, MissionDefinitionError, Objective, ObjectiveTrigger,
    StationType,
};

pub const EL_DESPERTAR: MissionId = MissionId(1);
pub const EVALUAR_ESTADO_NAVE: ObjectiveId = ObjectiveId(1);
pub const RESTAURAR_ENERGIA: ObjectiveId = ObjectiveId(2);
pub const RESTABLECER_SOPORTE_VITAL: ObjectiveId = ObjectiveId(3);
pub const ACCEDER_PUENTE_MANDO: ObjectiveId = ObjectiveId(4);
pub const TOMAR_CONTROL_NAVE: ObjectiveId = ObjectiveId(5);
pub const INSPECCIONAR_SISTEMAS_AUXILIARES: ObjectiveId = ObjectiveId(6);

pub fn definition() -> Result<Mission, MissionDefinitionError> {
    let evaluate = Objective::new(
        EVALUAR_ESTADO_NAVE,
        EL_DESPERTAR,
        "Evaluar el estado de la nave",
        true,
        ObjectiveTrigger::ShipStatusEvaluated {
            role: ConsoleRole::ShipStatus,
        },
    );
    let energy = Objective::new(
        RESTAURAR_ENERGIA,
        EL_DESPERTAR,
        "Restaurar la energía",
        true,
        ObjectiveTrigger::ElectricalSystemRepaired,
    )
    .with_dependencies([EVALUAR_ESTADO_NAVE]);
    let life_support = Objective::new(
        RESTABLECER_SOPORTE_VITAL,
        EL_DESPERTAR,
        "Restablecer el soporte vital",
        true,
        ObjectiveTrigger::OxygenSystemRepaired,
    )
    .with_dependencies([EVALUAR_ESTADO_NAVE]);
    let bridge = Objective::new(
        ACCEDER_PUENTE_MANDO,
        EL_DESPERTAR,
        "Acceder al puente de mando",
        true,
        ObjectiveTrigger::DoorRoleOpened {
            role: DoorRole::CommandBridge,
        },
    )
    .with_dependencies([RESTAURAR_ENERGIA, RESTABLECER_SOPORTE_VITAL]);
    let control = Objective::new(
        TOMAR_CONTROL_NAVE,
        EL_DESPERTAR,
        "Tomar el control de la nave",
        true,
        ObjectiveTrigger::StationTypeOccupied {
            station_type: StationType::Piloting,
        },
    )
    .with_dependencies([ACCEDER_PUENTE_MANDO]);
    let auxiliary = Objective::new(
        INSPECCIONAR_SISTEMAS_AUXILIARES,
        EL_DESPERTAR,
        "Inspeccionar los sistemas auxiliares",
        false,
        ObjectiveTrigger::Manual,
    );

    Mission::new(EL_DESPERTAR, "El despertar", vec![evaluate, energy, life_support, bridge, control, auxiliary],)
    .map(|mission| {
        mission.with_description(
            "La tripulación de ECHO-7 despierta tras un incidente desconocido y debe recuperar el control de la nave.",
        )
    })
}