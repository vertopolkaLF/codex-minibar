//! Settings picked from a fixed list of options.

/// A value chosen from [`Choice::ALL`], which is also the order every picker
/// lists it in. Indices are positions in that list; they are never persisted.
pub trait Choice: Copy + PartialEq + 'static {
    const ALL: &'static [Self];

    fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|choice| *choice == self)
            .unwrap_or_default()
    }

    /// The option at `index`, or the first one when it is out of range.
    fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::ALL[0])
    }
}
