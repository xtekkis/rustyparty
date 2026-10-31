use serde::Serialize;

// Where a round is: players play their cards, the judge reads them, then the winner is shown.
// The game is over once someone reaches the points to win
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Submitting,
    Judging,
    RoundOver,
    GameOver,
}
