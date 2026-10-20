use serde_json::Value;

pub type PlayerId = u32;
pub type TimerId = u32;

// Why a player's action was refused, sent back to them as an error
#[derive(Debug, PartialEq)]
pub struct GameError {
    pub code: &'static str,
    pub message: &'static str,
}

// What a game asks the room to do: messages to send and timers to start
#[derive(Default)]
pub struct GameCtx {}

// The rules of one game. A game never touches sockets, real timers or the database:
// it reacts to events and writes what should happen into the GameCtx
pub trait Game: Send {
    // Called once, when this game wins the vote
    fn start(&mut self, players: &[PlayerId], ctx: &mut GameCtx);

    // A player did something, like playing a card
    fn handle_action(
        &mut self,
        player: PlayerId,
        action: Value,
        ctx: &mut GameCtx,
    ) -> Result<(), GameError>;

    // A timer this game asked for has run out
    fn handle_timer(&mut self, timer: TimerId, ctx: &mut GameCtx);

    // A late joiner, the game adds them at the start of its next round
    fn player_joined(&mut self, player: PlayerId, ctx: &mut GameCtx);

    // A player left for good: left, kicked, banned, or their grace period ran out
    fn player_left(&mut self, player: PlayerId, ctx: &mut GameCtx);

    // What this player is allowed to see right now
    fn view_for(&self, player: PlayerId) -> Value;

    fn is_finished(&self) -> bool;
}
