use serde::Serialize;

// Where a round is: players play their cards, then the judge reads them
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Submitting,
    Judging,
}
