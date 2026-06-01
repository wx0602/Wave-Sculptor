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
            Self::Left => "左声道",
            Self::Right => "右声道",
            Self::SplitStereo => "分离立体声",
        }
    }
}
