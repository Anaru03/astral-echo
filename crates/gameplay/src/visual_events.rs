use engine::{Duration, EntityId, Vec3};

pub const VISUAL_EVENT_DURATION: Duration = Duration(250);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameplayVisualEventKind {
    Attack,
    EnemyDefeated,
    ItemCollected,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameplayVisualEvent {
    pub id: u64,
    pub kind: GameplayVisualEventKind,
    pub source: Option<EntityId>,
    pub target: Option<EntityId>,
    pub position: Option<Vec3>,
    pub emitted_at: Duration,
    pub expires_at: Duration,
}

impl GameplayVisualEvent {
    pub const fn is_active_at(self, now: Duration) -> bool {
        now.0 < self.expires_at.0
    }
}