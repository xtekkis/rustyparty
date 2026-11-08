use std::time::Duration;

use serde_json::Value;
use tokio::sync::mpsc::WeakSender;
use tokio::time::Instant;

use super::actor::{RoomCommand, remind};
use super::{Room, RoomState, seconds_until};
use crate::games::{GameCtx, GameInfo};
use crate::protocol::ServerMessage;

// How long the final scores stay on screen
pub(super) const RESULTS_DURATION: Duration = Duration::from_secs(10);

impl Room {
    // Start a fresh game with everyone who has a seat
    pub(super) fn start_game(&mut self, info: &GameInfo, me: &WeakSender<RoomCommand>) {
        self.games_started += 1;
        let number = self.games_started;

        let mut game = (info.create)(&self.settings);
        let players: Vec<u32> = self.players.iter().map(|player| player.id).collect();
        let mut ctx = GameCtx::default();
        game.start(&players, &mut ctx);

        self.state = RoomState::InGame {
            game,
            number,
            ends_at: None,
        };
        tracing::info!("Game {} started in room {}", info.id, self.code);
        self.run_game_ctx(ctx, number, me);
    }

    // Pass a player's move to the game, or tell them why the game refused it
    pub(super) fn game_action(
        &mut self,
        player_id: u32,
        action: Value,
        me: &WeakSender<RoomCommand>,
    ) {
        let RoomState::InGame { game, number, .. } = &mut self.state else {
            self.send_error(player_id, "not_in_game", "No game is running");
            return;
        };
        let number = *number;

        let mut ctx = GameCtx::default();
        match game.handle_action(player_id, action, &mut ctx) {
            Ok(()) => self.run_game_ctx(ctx, number, me),
            // A refused action has no effects, whatever the game wrote into ctx is dropped
            Err(error) => self.send_error(player_id, error.code, error.message),
        }
    }

    // The game decides when they start playing, usually at its next round
    pub(super) fn tell_game_about_late_joiner(
        &mut self,
        player_id: u32,
        me: &WeakSender<RoomCommand>,
    ) {
        let RoomState::InGame { game, number, .. } = &mut self.state else {
            return;
        };
        let number = *number;
        let mut ctx = GameCtx::default();
        game.player_joined(player_id, &mut ctx);
        self.run_game_ctx(ctx, number, me);
    }

    // Someone is gone for good, the game may need to cancel a round or end
    pub(super) fn tell_game_about_leaver(&mut self, player_id: u32, me: &WeakSender<RoomCommand>) {
        let RoomState::InGame { game, number, .. } = &mut self.state else {
            return;
        };
        let number = *number;
        let mut ctx = GameCtx::default();
        game.player_left(player_id, &mut ctx);
        self.run_game_ctx(ctx, number, me);
    }

    // Carry out what a game asked for: send its messages and start its timers
    pub(super) fn run_game_ctx(
        &mut self,
        ctx: GameCtx,
        game_number: u32,
        me: &WeakSender<RoomCommand>,
    ) {
        self.send_game_messages(ctx.outgoing);
        for (timer, after) in ctx.timers {
            remind(me, after, RoomCommand::GameTimer { game_number, timer });
            if let RoomState::InGame { ends_at, .. } = &mut self.state {
                *ends_at = Some(Instant::now() + after);
            }
        }
        self.send_game_views();
    }

    // Every player gets the game as they are allowed to see it, never someone else's
    pub(super) fn send_game_views(&self) {
        for player in &self.players {
            self.send_game_view_to(player.id);
        }
    }

    pub(super) fn send_game_view_to(&self, player_id: u32) {
        let RoomState::InGame { game, ends_at, .. } = &self.state else {
            return;
        };
        let view = game.view_for(player_id);
        // A finished game counts down to nothing, whatever timer was still running
        let ends_in = ends_at.filter(|_| !game.is_finished()).map(seconds_until);
        self.send_to_player(player_id, ServerMessage::GameView { view, ends_in });
    }

    // Send everyone what the game asked to send, each one wrapped as a game event
    pub(super) fn send_game_messages(&self, outgoing: Vec<Value>) {
        for event in outgoing {
            self.broadcast(&ServerMessage::GameEvent { event });
        }
    }

    // The game says it's over: show the results for a while
    pub(super) fn finish_game_if_over(&mut self, me: &WeakSender<RoomCommand>) {
        let RoomState::InGame { game, number, .. } = &self.state else {
            return;
        };
        if !game.is_finished() {
            return;
        }

        let number = *number;
        self.state = RoomState::Results { number };
        tracing::info!("Game {number} finished in room {}", self.code);
        self.broadcast(&ServerMessage::Results {
            ends_in: RESULTS_DURATION.as_secs(),
        });
        remind(me, RESULTS_DURATION, RoomCommand::ResultsOver { number });
    }
}
