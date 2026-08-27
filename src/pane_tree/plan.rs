use super::ratio::Ratio;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    First,
    Second,
}

#[derive(Debug, PartialEq)]
pub struct Adjustment {
    pub path: Vec<Side>,
    pub ratio: Ratio,
}
