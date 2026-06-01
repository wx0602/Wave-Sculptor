#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveformMode {
    Mixed,
    Left,
    Right,
    SplitStereo,
}

impl WaveformMode {
    pub const ALL: [Self; 4] = [Self::Mixed, Self::Left, Self::Right, Self::SplitStereo];

    pub fn label(self) -> &'static str {
        match self {
            Self::Mixed => "混合",
            Self::Left => "向左",
            Self::Right => "向右",
            Self::SplitStereo => "双声道分离",
        }
    }
}
