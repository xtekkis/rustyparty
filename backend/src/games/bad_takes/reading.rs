// The bag: the judge pulls the answers out one at a time, then shows them all
use rand::seq::SliceRandom;
use serde_json::json;

use super::state::Phase;
use super::{BadTakes, IDLE_PULL_DURATION, PULL_DURATION, Timer};
use crate::games::{GameCtx, GameError, PlayerId, TimerId};

impl BadTakes {
    // The answers go into a bag, for the judge to pull out one at a time
    pub(super) fn start_reading(&mut self, ctx: &mut GameCtx) {
        // Shuffled, so the judge can't tell whose cards came in first
        self.reading_order = self.submitted_players();
        self.reading_order.shuffle(&mut self.rng);
        self.phase = Phase::Reading;
        ctx.broadcast(json!({
            "event": "reading_started",
            "answers": self.reading_order.len(),
        }));
        self.start_pull_timer(ctx);
    }

    // Pulling and showing the cards are for the judge, while the answers are being read
    fn check_judge_is_reading(&self, player: PlayerId) -> Result<(), GameError> {
        if self.phase != Phase::Reading {
            return Err(GameError {
                code: "not_reading",
                message: "The answers aren't being read right now",
            });
        }
        if self.judge != Some(player) {
            return Err(GameError {
                code: "not_the_judge",
                message: "Only the judge handles the cards",
            });
        }
        Ok(())
    }

    pub(super) fn pull(&mut self, player: PlayerId, ctx: &mut GameCtx) -> Result<(), GameError> {
        self.check_judge_is_reading(player)?;
        if self.in_bag() == 0 {
            return Err(GameError {
                code: "bag_empty",
                message: "Every card is out of the bag",
            });
        }
        self.judge_tapped = true;
        self.judge_missed_tap = false;
        self.pull_next(ctx);
        Ok(())
    }

    pub(super) fn show_all(
        &mut self,
        player: PlayerId,
        ctx: &mut GameCtx,
    ) -> Result<(), GameError> {
        self.check_judge_is_reading(player)?;
        if self.in_bag() > 0 {
            return Err(GameError {
                code: "cards_in_bag",
                message: "There are still cards in the bag",
            });
        }
        self.judge_tapped = true;
        self.start_judging(ctx);
        Ok(())
    }

    // How many answers are still waiting in the bag
    pub(super) fn in_bag(&self) -> usize {
        self.reading_order.len() - self.pulled
    }

    // Everyone sees the next answer at the same time, while the judge reads it out loud
    fn pull_next(&mut self, ctx: &mut GameCtx) {
        let next = self.reading_order.get(self.pulled);
        if let Some(cards) = next.and_then(|player| self.submissions.get(player)) {
            ctx.broadcast(json!({ "event": "answer_pulled", "cards": cards }));
            self.pulled += 1;
        }
        self.start_pull_timer(ctx);
    }

    // The judge stopped tapping: the next card comes out, or they're all shown after the last
    pub(super) fn pull_time_up(&mut self, ctx: &mut GameCtx) {
        self.judge_missed_tap = true;
        if self.in_bag() > 0 {
            self.pull_next(ctx);
        } else {
            self.start_judging(ctx);
        }
    }

    // Each tap starts a new auto-pull timer, so each one gets its own id
    pub(super) fn pull_timer_id(&self) -> TimerId {
        self.timer_id(Timer::Pull) + self.pull_timers
    }

    // The older auto-pull timers can't be cancelled, but their ids no longer count
    fn start_pull_timer(&mut self, ctx: &mut GameCtx) {
        self.pull_timers += 1;
        let wait = if self.judge_missed_tap {
            IDLE_PULL_DURATION
        } else {
            PULL_DURATION
        };
        ctx.start_timer(self.pull_timer_id(), wait);
    }
}
