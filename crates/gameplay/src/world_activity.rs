use crate::{
    combat::EnemyConfiguration,
    knowledge::KnowledgeTarget,
    navigation::DestinationId,
    types::ItemId,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldCategory {
    Trade,
    Resource,
    Combat,
    Story,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TradeOffer {
    pub item: ItemId,
    pub buy_price: u64,
    pub sell_price: u64,
    pub stock: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TradeMarket {
    pub offers: Vec<TradeOffer>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceSite {
    pub item: ItemId,
    pub available: u32,
    pub extraction_amount: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CombatZone {
    pub configuration: EnemyConfiguration,
    pub active_enemy: Option<engine::EntityId>,
    pub resolved: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorySite {
    pub target: KnowledgeTarget,
    pub interacted: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WorldActivity {
    Trade(TradeMarket),
    Resource(ResourceSite),
    Combat(CombatZone),
    Story(StorySite),
}

impl WorldActivity {
    pub const fn category(&self) -> WorldCategory {
        match self {
            Self::Trade(_) => WorldCategory::Trade,
            Self::Resource(_) => WorldCategory::Resource,
            Self::Combat(_) => WorldCategory::Combat,
            Self::Story(_) => WorldCategory::Story,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionalWorld {
    pub destination: DestinationId,
    pub interaction_range: f32,
    pub activities: Vec<WorldActivity>,
}

impl FunctionalWorld {
    pub fn new(
        destination: DestinationId,
        interaction_range: f32,
        activities: Vec<WorldActivity>,
    ) -> Self {
        Self {
            destination,
            interaction_range,
            activities,
        }
    }

    pub fn validate(&self) -> Result<(), FunctionalWorldError> {
        if self.destination.value() == 0 {
            return Err(FunctionalWorldError::InvalidDestination);
        }
        if !self.interaction_range.is_finite() || self.interaction_range <= 0.0 {
            return Err(FunctionalWorldError::InvalidRange);
        }
        if self.activities.is_empty() {
            return Err(FunctionalWorldError::EmptyActivities);
        }
        let mut categories = Vec::new();
        for activity in &self.activities {
            if categories.contains(&activity.category()) {
                return Err(FunctionalWorldError::DuplicateCategory);
            }
            categories.push(activity.category());
            match activity {
                WorldActivity::Trade(market) if market.offers.is_empty() => {
                    return Err(FunctionalWorldError::InvalidTrade)
                }
                WorldActivity::Trade(market) => {
                    let mut items = Vec::new();
                    for offer in &market.offers {
                        if offer.item.0 == 0
                            || offer.buy_price == 0
                            || offer.sell_price == 0
                            || offer.stock == 0
                            || items.contains(&offer.item)
                        {
                            return Err(FunctionalWorldError::InvalidTrade);
                        }
                        items.push(offer.item);
                    }
                }
                WorldActivity::Resource(site)
                    if site.item.0 == 0
                        || site.available == 0
                        || site.extraction_amount == 0 =>
                {
                    return Err(FunctionalWorldError::InvalidResource)
                }
                WorldActivity::Resource(_) => {}
                WorldActivity::Combat(zone) => {
                    zone.configuration
                        .validate()
                        .map_err(|_| FunctionalWorldError::InvalidCombat)?;
                }
                WorldActivity::Story(_) => {}
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FunctionalWorldError {
    InvalidDestination,
    DuplicateDestination,
    DestinationNotFound,
    DestinationNotDiscovered,
    InvalidRange,
    EmptyActivities,
    DuplicateCategory,
    InvalidTrade,
    InvalidResource,
    InvalidCombat,
    InvalidStory,
    ItemDefinitionNotFound,
    KnowledgeTargetNotFound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldActivityError {
    WorldNotFound,
    DestinationNotDiscovered,
    PlayerNotFound,
    PlayerInactive,
    ShipNotOperational,
    OutOfRange,
    ActivityNotFound,
    InvalidQuantity,
    InvalidItem,
    InsufficientFunds,
    PriceOverflow,
    InventoryCapacity,
    InventoryInsufficient,
    ResourceUnavailable,
    CombatAlreadyActivated,
    CombatResolved,
    StoryAlreadyInteracted,
    StoryKnowledgeUnavailable,
    EnemyCreationFailed,
    UpgradeAlreadyInstalled,
}