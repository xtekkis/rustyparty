use serde::Serialize;

// Where a round is: players play their cards, the judge pulls them out of a bag one at a time
// and shows them all, then picks one and the winner is shown. The game is over once someone
// reaches the points to win
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Submitting,
    Reading,
    Judging,
    RoundOver,
    GameOver,
}
