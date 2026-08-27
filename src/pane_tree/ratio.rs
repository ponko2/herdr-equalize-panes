use std::{error::Error, fmt, num::NonZeroU32};

const TOLERANCE: f64 = 1e-3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ratio(f64);

impl Ratio {
    pub fn from_leaf_counts(first: NonZeroU32, second: NonZeroU32) -> Self {
        let first = f64::from(first.get());
        let second = f64::from(second.get());
        Self(first / (first + second))
    }

    pub fn as_f64(self) -> f64 {
        self.0
    }

    pub fn approx_eq(self, other: Self) -> bool {
        (self.0 - other.0).abs() <= TOLERANCE
    }
}

impl TryFrom<f64> for Ratio {
    type Error = OutOfRange;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if value.is_finite() && (0.0..=1.0).contains(&value) {
            Ok(Self(value))
        } else {
            Err(OutOfRange(value))
        }
    }
}

#[derive(Debug)]
pub struct OutOfRange(f64);

impl fmt::Display for OutOfRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} is not a share of a split between 0 and 1", self.0)
    }
}

impl Error for OutOfRange {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ratio_accepts_a_finite_fraction_of_the_split() {
        assert!(Ratio::try_from(0.5).is_ok());
        assert!(Ratio::try_from(0.0).is_ok());
        assert!(Ratio::try_from(1.0).is_ok());
    }

    fn exactly(value: f64) -> Ratio {
        Ratio::try_from(value).expect("the fixture is a valid ratio")
    }

    fn leaves(count: u32) -> NonZeroU32 {
        NonZeroU32::new(count).expect("a subtree has at least one leaf")
    }

    #[test]
    fn each_side_gets_the_share_its_pane_count_deserves() {
        assert_eq!(Ratio::from_leaf_counts(leaves(1), leaves(1)), exactly(0.5));
        assert_eq!(
            Ratio::from_leaf_counts(leaves(1), leaves(2)),
            exactly(1.0 / 3.0)
        );
        assert_eq!(Ratio::from_leaf_counts(leaves(3), leaves(1)), exactly(0.75));
    }

    #[test]
    fn ratios_within_a_cell_of_rounding_are_the_same_ratio() {
        assert!(exactly(0.5).approx_eq(exactly(0.5005)));
        assert!(!exactly(0.5).approx_eq(exactly(0.502)));
    }

    #[test]
    fn a_ratio_rejects_what_cannot_describe_a_split() {
        assert!(Ratio::try_from(-0.1).is_err());
        assert!(Ratio::try_from(1.1).is_err());
        assert!(Ratio::try_from(f64::NAN).is_err());
        assert!(Ratio::try_from(f64::INFINITY).is_err());
    }
}
