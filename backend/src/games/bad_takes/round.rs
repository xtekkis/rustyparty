// The round cycle: dealing, the next judge, skipped and cancelled rounds, and the end of the game
use serde_json::json;

use super::deck::Deck;
use super::state::Phase;
use super::{BadTakes, HAND_SIZE, SUBMIT_DURATION, Timer};
use crate::games::{GameCtx, PlayerId};

impl BadTakes {
    // Everyone has 7 cards again, then a new judge reads out a new prompt
    pub(super) fn start_round(&mut self, ctx: &mut GameCtx) {
        self.round += 1;
        self.phase = Phase::Submitting;
        // Late joiners go to the end of the judge rotation, while there's a free seat
        while self.players.len() < self.max_players && !self.waiting.is_empty() {
            let player = self.waiting.remove(0);
            self.players.push(player);
            self.seating.push(player);
            self.scores.insert(player, 0);
        }

        // Last round's cards stay on the table until now, so everyone can see the reveal
        self.discard
            .extend(self.submissions.drain().flat_map(|(_, cards)| cards));
        for player in self.players.clone() {
            self.deal_to(player);
        }
        self.reading_order.clear();
        self.pulled = 0;
        self.pull_timers = 0;
        self.judge_tapped = false;
        self.judge_missed_tap = false;
        self.winner = None;
        self.judge = self.next_judge();
        // Last round's prompt is used up. Out of prompts, the used ones become a new deck
        self.used_prompts.extend(self.prompt.take());
        if self.prompts.is_empty() {
            let used = std::mem::take(&mut self.used_prompts);
            self.prompts = Deck::shuffled(used, &mut self.rng);
        }
        self.prompt = self.prompts.draw();
        ctx.broadcast(json!({
            "event": "round_started",
            "round": self.round,
            "judge": self.judge,
            "prompt": self.prompt,
            "submit_for": SUBMIT_DURATION.as_secs(),
        }));
        ctx.start_timer(self.timer_id(Timer::Submit), SUBMIT_DURATION);
    }

    // The player after the current judge in join order, back to the first after the last
    fn next_judge(&self) -> Option<PlayerId> {
        let after_judge = self
            .judge
            .and_then(|judge| self.seating.iter().position(|&player| player == judge))
            .map_or(0, |seat| seat + 1);
        // Walk round the table from there, skipping anyone who has left
        (0..self.seating.len())
            .map(|step| self.seating[(after_judge + step) % self.seating.len()])
            .find(|player| self.players.contains(player))
    }

    // Fill a player's hand up to 7 cards
    fn deal_to(&mut self, player: PlayerId) {
        let hand = self.hands.entry(player).or_default();
        while hand.len() < HAND_SIZE {
            // The deck ran out: the played cards are shuffled into a new one
            if self.answers.is_empty() && !self.discard.is_empty() {
                let played = std::mem::take(&mut self.discard);
                self.answers = Deck::shuffled(played, &mut self.rng);
            }
            let Some(card) = self.answers.draw() else {
                break;
            };
            hand.push(card);
        }
    }

    // The judge left mid-round: everyone takes their cards back and the next judge starts again
    pub(super) fn cancel_round(&mut self, ctx: &mut GameCtx) {
        for (player, cards) in self.submissions.drain() {
            if let Some(hand) = self.hands.get_mut(&player) {
                hand.extend(cards);
            }
        }
        ctx.broadcast(json!({ "event": "round_cancelled" }));
        self.start_round(ctx);
    }

    // Nothing to judge: no point this round, and the next judge starts a new one
    pub(super) fn skip_round(&mut self, ctx: &mut GameCtx) {
        ctx.broadcast(json!({ "event": "round_skipped" }));
        self.start_round(ctx);
    }

    // The reveal is over: the game ends, or the next round starts
    pub(super) fn reveal_over(&mut self, ctx: &mut GameCtx) {
        if let Some(winner) = self.game_winner() {
            self.end_game(Some(winner), "points_to_win", ctx);
        } else {
            self.start_round(ctx);
        }
    }

    // The final scores, and why the game ended
    pub(super) fn end_game(&mut self, winner: Option<PlayerId>, reason: &str, ctx: &mut GameCtx) {
        self.phase = Phase::GameOver;
        self.winner = winner;
        ctx.broadcast(json!({
            "event": "game_over",
            "reason": reason,
            "winner": winner,
            "cards": self.winning_cards(),
            "scores": self.scores,
        }));
    }

    // First to the room's points to win takes the game
    pub(super) fn game_winner(&self) -> Option<PlayerId> {
        self.scores
            .iter()
            .find(|&(_, &points)| points >= self.points_to_win)
            .map(|(&player, _)| player)
    }
}
