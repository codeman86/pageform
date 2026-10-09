//! Grid presets. Snap uses the minor spacing of whichever preset is active.
//!
//! | Preset | Minor | Major |
//! | --- | --- | --- |
//! | Small | 0.0625 in (4.5 pt) | 0.25 in (18 pt) |
//! | Medium (default) | 0.125 in (9 pt) | 0.5 in (36 pt) |
//! | Large | 0.25 in (18 pt) | 1.0 in (72 pt) |
//!
//! Major lines are a visual emphasis every four minor steps. Hiding the grid
//! does not change the snap spacing.

/// One PDF point is 1/72 inch.
pub const PT_PER_INCH: f32 = 72.0;

/// How dense the canvas grid is, and which spacing snap uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum GridSize {
    /// Denser grid. Minor 1/16 in, major 1/4 in.
    Small,
    /// Default grid. Minor 1/8 in, major 1/2 in.
    #[default]
    Medium,
    /// Wider grid. Minor 1/4 in, major 1 in.
    Large,
}

impl GridSize {
    pub const ALL: [GridSize; 3] = [GridSize::Small, GridSize::Medium, GridSize::Large];

    pub fn label(self) -> &'static str {
        match self {
            GridSize::Small => "Small",
            GridSize::Medium => "Medium",
            GridSize::Large => "Large",
        }
    }

    /// Minor spacing in inches. Snap uses this value.
    pub fn minor_inches(self) -> f32 {
        match self {
            GridSize::Small => 0.0625,
            GridSize::Medium => 0.125,
            GridSize::Large => 0.25,
        }
    }

    /// Major spacing in inches. Drawn heavier; not used for snap.
    pub fn major_inches(self) -> f32 {
        match self {
            GridSize::Small => 0.25,
            GridSize::Medium => 0.5,
            GridSize::Large => 1.0,
        }
    }

    pub fn minor_pt(self) -> f32 {
        self.minor_inches() * PT_PER_INCH
    }

    pub fn major_pt(self) -> f32 {
        self.major_inches() * PT_PER_INCH
    }

    pub fn minor_label(self) -> &'static str {
        match self {
            GridSize::Small => "1/16 in",
            GridSize::Medium => "1/8 in",
            GridSize::Large => "1/4 in",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1e-4, "{actual} != {expected}");
    }

    #[test]
    fn presets_match_the_design_note() {
        close(GridSize::Small.minor_pt(), 4.5);
        close(GridSize::Small.major_pt(), 18.0);
        close(GridSize::Medium.minor_pt(), 9.0);
        close(GridSize::Medium.major_pt(), 36.0);
        close(GridSize::Large.minor_pt(), 18.0);
        close(GridSize::Large.major_pt(), 72.0);
        assert_eq!(GridSize::default(), GridSize::Medium);
        for size in GridSize::ALL {
            close(size.major_pt() / size.minor_pt(), 4.0);
        }
    }
}
