// Picking the winner and scoring the point
use rand::seq::IndexedRandom;
use serde_json::json;

use super::deck::{AnswerCard, CardId};
use super::state::Phase;
use super::{BadTakes, IDLE_JUDGE_DURATION, JUDGE_DURATION, REVEAL_DURATION, Timer};
use crate::games::{GameCtx, GameError, PlayerId};

impl BadTakes {
    // All the answers are shown together, and the judge's time to pick starts
    pub(super) fn start_judging(&mut self, ctx: &mut GameCtx) {
        self.phase = Phase::Judging;
        let judge_for = if self.judge_tapped {
            JUDGE_DURATION
        } else {
            IDLE_JUDGE_DURATION
        };
        ctx.broadcast(json!({
            "event": "judging_started",
            "judge_for": judge_for.as_secs(),
        }));
        ctx.start_timer(self.timer_id(Timer::Judge), judge_for);
    }

    pub(super) fn pick(
        &mut self,
        player: PlayerId,
        card: CardId,
        ctx: &mut GameCtx,
    ) -> Result<(), GameError> {
        if self.phase != Phase::Judging {
            return Err(GameError {
                code: "not_judging",
                message: "There's nothing to pick right now",
            });
        }
        if self.judge != Some(player) {
            return Err(GameError {
                code: "not_the_judge",
                message: "Only the judge picks the winner",
            });
        }

        // Any card of an answer picks the whole answer
        let owner = self.reading_order.iter().copied().find(|owner| {
            self.submissions
                .get(owner)
                .is_some_and(|cards| cards.iter().any(|played| played.id == card))
        });
        let Some(winner) = owner else {
            return Err(GameError {
                code: "not_a_choice",
                message: "That card isn't one of the answers",
            });
        };
        self.award_point(winner, false, ctx);
        Ok(())
    }

    // The judge took too long, so a random answer wins and still gets the point
    pub(super) fn judge_time_up(&mut self, ctx: &mut GameCtx) {
        if let Some(&winner) = self.reading_order.choose(&mut self.rng) {
            self.award_point(winner, true, ctx);
        }
    }

    fn award_point(&mut self, winner: PlayerId, judge_timed_out: bool, ctx: &mut GameCtx) {
        self.winner = Some(winner);
        let points = self.scores.entry(winner).or_default();
        *points += 1;
        let points = *points;
        self.phase = Phase::RoundOver;

        // Only the winning answer gets a name, the rest stay anonymous
        ctx.broadcast(json!({
            "event": "winner_revealed",
            "winner": winner,
            "cards": self.winning_cards(),
            "points": points,
            "reveal_for": REVEAL_DURATION.as_secs(),
            "last_round": self.game_winner().is_some(),
            "judge_timed_out": judge_timed_out,
        }));
        ctx.start_timer(self.timer_id(Timer::Reveal), REVEAL_DURATION);
    }

    // The winner's answer. It stops being secret once the judge has picked it
    pub(super) fn winning_cards(&self) -> Option<&Vec<AnswerCard>> {
        self.winner.and_then(|winner| self.submissions.get(&winner))
    }

    // The answers out of the bag so far, in reading order, with nothing to say whose they are
    pub(super) fn cards_to_judge(&self) -> Vec<&Vec<AnswerCard>> {
        self.reading_order
            .iter()
            .take(self.pulled)
            .filter_map(|player| self.submissions.get(player))
            .collect()
    }
}
