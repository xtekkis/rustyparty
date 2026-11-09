// Playing cards, then moving on once everyone has played or time is up
use std::collections::HashSet;

use serde_json::json;

use super::BadTakes;
use super::deck::CardId;
use super::state::Phase;
use crate::games::{GameCtx, GameError, PlayerId};

impl BadTakes {
    pub(super) fn submit(
        &mut self,
        player: PlayerId,
        card_ids: Vec<CardId>,
        ctx: &mut GameCtx,
    ) -> Result<(), GameError> {
        if self.phase != Phase::Submitting {
            return Err(GameError {
                code: "not_submitting",
                message: "Cards can't be played right now",
            });
        }
        // The judge picks the winner, so they can't play a card of their own
        if self.judge == Some(player) {
            return Err(GameError {
                code: "judge_cannot_submit",
                message: "You're the judge this round",
            });
        }
        if self.submissions.contains_key(&player) {
            return Err(GameError {
                code: "already_submitted",
                message: "You already played your cards this round",
            });
        }

        // One card per gap, a pick 2 prompt fills its gaps in the order the cards were picked
        let gaps = self.prompt.as_ref().map(|prompt| usize::from(prompt.pick));
        if gaps != Some(card_ids.len()) {
            return Err(GameError {
                code: "wrong_card_count",
                message: "Play one card for each gap in the prompt",
            });
        }

        // Late joiners have no hand until the next round
        let Some(hand) = self.hands.get_mut(&player) else {
            return Err(GameError {
                code: "not_playing",
                message: "You join at the next round",
            });
        };

        // Check every card before taking any, so a refused submission leaves the hand as it was
        let all_held = card_ids
            .iter()
            .all(|&id| hand.iter().any(|card| card.id == id));
        let all_different = card_ids.iter().collect::<HashSet<_>>().len() == card_ids.len();
        if !all_held || !all_different {
            return Err(GameError {
                code: "card_not_in_hand",
                message: "You don't have that card",
            });
        }

        // Take the cards out of the hand, in the order they were picked
        let mut cards = Vec::new();
        for id in card_ids {
            let index = hand
                .iter()
                .position(|card| card.id == id)
                .expect("checked above");
            cards.push(hand.remove(index));
        }
        self.submissions.insert(player, cards);

        // Everyone sees who has played, nobody sees what
        ctx.broadcast(json!({ "event": "submitted", "player": player }));
        self.judge_when_everyone_submitted(ctx);
        Ok(())
    }

    // Once every player but the judge has played, the judge reads the cards
    pub(super) fn judge_when_everyone_submitted(&mut self, ctx: &mut GameCtx) {
        if self.phase != Phase::Submitting || self.submissions.is_empty() {
            return;
        }
        let everyone_played = self
            .players
            .iter()
            .filter(|&&player| Some(player) != self.judge)
            .all(|player| self.submissions.contains_key(player));
        if everyone_played {
            self.start_reading(ctx);
        }
    }

    // Whoever hasn't played by now sits this round out
    pub(super) fn submit_time_up(&mut self, ctx: &mut GameCtx) {
        if self.submissions.is_empty() {
            self.skip_round(ctx);
        } else {
            self.start_reading(ctx);
        }
    }

    // Who has played this round, in a fixed order
    pub(super) fn submitted_players(&self) -> Vec<PlayerId> {
        let mut players: Vec<PlayerId> = self.submissions.keys().copied().collect();
        players.sort();
        players
    }
}
