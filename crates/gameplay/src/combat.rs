use engine::{Duration, EntityId, Vec3};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttackType {
    Kinetic,
    Energy,
    Plasma,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeaponConfiguration {
    pub attack_type: AttackType,
    pub damage: u32,
    pub range: u32,
    pub cooldown: Duration,
}

impl WeaponConfiguration {
    pub const fn new(attack_type: AttackType, damage: u32, range: u32, cooldown: Duration) -> Self {
        Self {
            attack_type,
            damage,
            range,
            cooldown,
        }
    }

    pub fn validate(self) -> Result<(), CombatConfigurationError> {
        if self.damage == 0 || self.range == 0 || self.cooldown.0 == 0 {
            return Err(CombatConfigurationError::InvalidWeapon);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Shield {
    pub current: u32,
    pub maximum: u32,
    pub weakness: AttackType,
}

impl Shield {
    pub const fn new(maximum: u32, weakness: AttackType) -> Self {
        Self {
            current: maximum,
            maximum,
            weakness,
        }
    }

    pub const fn is_active(self) -> bool {
        self.current > 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnemyArchetype {
    Shielded,
    Shooter,
    Charger,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnemyConfiguration {
    pub archetype: EnemyArchetype,
    pub position: Vec3,
    pub maximum_health: u32,
    pub weapon: WeaponConfiguration,
    pub shield: Option<Shield>,
    pub mobility: u32,
}

impl EnemyConfiguration {
    pub fn for_archetype(archetype: EnemyArchetype, position: Vec3) -> Self {
        let (maximum_health, weapon, shield, mobility) = match archetype {
            EnemyArchetype::Shielded => (
                100,
                WeaponConfiguration::new(AttackType::Kinetic, 10, 3, Duration(1_000)),
                Some(Shield::new(50, AttackType::Energy)),
                4,
            ),
            EnemyArchetype::Shooter => (
                60,
                WeaponConfiguration::new(AttackType::Energy, 8, 20, Duration(800)),
                None,
                10,
            ),
            EnemyArchetype::Charger => (
                150,
                WeaponConfiguration::new(AttackType::Kinetic, 20, 2, Duration(1_500)),
                None,
                5,
            ),
        };
        Self {
            archetype,
            position,
            maximum_health,
            weapon,
            shield,
            mobility,
        }
    }

    pub fn validate(self) -> Result<(), CombatConfigurationError> {
        if !self.position.x.is_finite()
            || !self.position.y.is_finite()
            || !self.position.z.is_finite()
        {
            return Err(CombatConfigurationError::InvalidPosition);
        }
        if self.maximum_health == 0 || self.mobility == 0 {
            return Err(CombatConfigurationError::InvalidEnemy);
        }
        self.weapon.validate()?;
        if let Some(shield) = self.shield {
            if shield.maximum == 0 || shield.current > shield.maximum {
                return Err(CombatConfigurationError::InvalidShield);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombatConfigurationError {
    InvalidPosition,
    InvalidEnemy,
    InvalidWeapon,
    InvalidShield,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Combatant {
    pub entity: EntityId,
    pub is_player: bool,
    pub active: bool,
    pub maximum_health: u32,
    pub current_health: u32,
    pub archetype: Option<EnemyArchetype>,
    pub position: Vec3,
    pub weapon: WeaponConfiguration,
    pub shield: Option<Shield>,
    pub mobility: u32,
    pub last_attack_at: Option<Duration>,
}

impl Combatant {
    pub fn player(entity: EntityId) -> Self {
        Self {
            entity,
            is_player: true,
            active: true,
            maximum_health: 100,
            current_health: 100,
            archetype: None,
            position: Vec3::ZERO,
            weapon: WeaponConfiguration::new(AttackType::Kinetic, 15, 10, Duration(1_000)),
            shield: None,
            mobility: 10,
            last_attack_at: None,
        }
    }

    pub fn enemy(entity: EntityId, config: EnemyConfiguration) -> Self {
        Self {
            entity,
            is_player: false,
            active: true,
            maximum_health: config.maximum_health,
            current_health: config.maximum_health,
            archetype: Some(config.archetype),
            position: config.position,
            weapon: config.weapon,
            shield: config.shield,
            mobility: config.mobility,
            last_attack_at: None,
        }
    }

    pub const fn is_defeated(self) -> bool {
        self.current_health == 0
    }
}