use gameplay::types::{MissionId, ObjectiveId};
use gameplay::{EnemyArchetype, Mission, MissionDefinitionError, Objective, ObjectiveTrigger};

pub const LA_DEFENSA_IMPOSIBLE: MissionId = MissionId(4);
pub const COMPROBAR_LA_DEFENSA: ObjectiveId = ObjectiveId(14);
pub const DESACTIVAR_EL_ESCUDO: ObjectiveId = ObjectiveId(15);
pub const NEUTRALIZAR_EL_ESCUDADO: ObjectiveId = ObjectiveId(16);
pub const RECUPERAR_LA_RECOMPENSA: ObjectiveId = ObjectiveId(17);

pub fn definition() -> Result<Mission, MissionDefinitionError> {
    let check_defense = Objective::new(
        COMPROBAR_LA_DEFENSA,
        LA_DEFENSA_IMPOSIBLE,
        "Comprobar la defensa del enemigo",
        true,
        ObjectiveTrigger::ShieldDamaged { enemy: None },
    );
    let disable_shield = Objective::new(
        DESACTIVAR_EL_ESCUDO,
        LA_DEFENSA_IMPOSIBLE,
        "Desactivar el escudo",
        true,
        ObjectiveTrigger::ShieldDisabled { enemy: None },
    )
    .with_dependencies([COMPROBAR_LA_DEFENSA]);
    let defeat_shielded = Objective::new(
        NEUTRALIZAR_EL_ESCUDADO,
        LA_DEFENSA_IMPOSIBLE,
        "Neutralizar al enemigo escudado",
        true,
        ObjectiveTrigger::EnemiesDefeated {
            archetype: Some(EnemyArchetype::Shielded),
            count: 1,
        },
    )
    .with_dependencies([DESACTIVAR_EL_ESCUDO]);
    let recover_reward = Objective::new(
        RECUPERAR_LA_RECOMPENSA,
        LA_DEFENSA_IMPOSIBLE,
        "Recuperar la recompensa",
        true,
        ObjectiveTrigger::ItemsCollected {
            item: gameplay::types::ItemId(1),
            amount: 1,
        },
    )
    .with_dependencies([NEUTRALIZAR_EL_ESCUDADO]);

    Mission::new(
        LA_DEFENSA_IMPOSIBLE,
        "La defensa imposible",
        vec![
            check_defense,
            disable_shield,
            defeat_shielded,
            recover_reward,
        ],
    )
    .map(|mission| {
        mission.with_description(
            "ECHO-7 descubre una entidad protegida y debe adaptar su armamento para neutralizarla.",
        )
    })
}