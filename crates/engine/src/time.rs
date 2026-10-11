#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Tick(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct Duration(pub u64);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GameTime {
    pub tick: Tick,
    pub elapsed: Duration,
}

impl GameTime {
    pub const fn new() -> Self {
        Self {
            tick: Tick(0),
            elapsed: Duration(0),
        }
    }
}