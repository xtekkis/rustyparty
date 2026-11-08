// Bad Takes: an adult fill-in-the-blank card game
pub mod deck;
pub mod state;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use rand::rngs::StdRng;
use rand::seq::{IndexedRandom, SliceRandom};
use serde::Deserialize;
use serde_json::{Value, json};

use deck::{AnswerCard, CardId, Deck, PromptCard};
use state::Phase;

use super::{Game, GameCtx, GameError, GameInfo, PlayerId, TimerId};
use crate::protocol::RoomSettings;

pub const HAND_SIZE: usize = 7;
const MIN_PLAYERS: usize = 3;
const MAX_PLAYERS: usize = 10;

// What a timer is for. Timers can't be cancelled, so each id also carries its round,
// and a timer left over from an earlier round is ignored
#[derive(Clone, Copy)]
enum Timer {
    Submit = 1,
    Reveal = 2,
    Judge = 3,
    Pull = 4,
}

// How long players have to play their cards
const SUBMIT_DURATION: Duration = Duration::from_secs(60);
// How long the judge can leave the next step, before it happens by itself
pub(crate) const PULL_DURATION: Duration = Duration::from_secs(15);
// The same, once the judge has missed a tap and seems to be away
pub(crate) const IDLE_PULL_DURATION: Duration = Duration::from_secs(6);
// How long the judge has to pick, before a random answer wins
pub(crate) const JUDGE_DURATION: Duration = Duration::from_secs(45);
// The same, for a judge who hasn't tapped at all this round
pub(crate) const IDLE_JUDGE_DURATION: Duration = Duration::from_secs(20);
// The winning sentence and the scores, before the next round or the final results
pub(crate) const REVEAL_DURATION: Duration = Duration::from_secs(4);

// The registry entry, built at startup because the player limit depends on the deck
pub fn info() -> GameInfo {
    GameInfo {
        id: "bad_takes",
        name: "Bad Takes",
        description: "Fill in the blank with your funniest card. The judge picks the winner.",
        min_players: MIN_PLAYERS,
        max_players: max_players(deck::starter_deck().answers.len()),
        create,
    }
}

// Half the answers stay in the draw pile, so hands get fresh cards instead of the ones just played
fn max_players(answer_count: usize) -> usize {
    (answer_count / (HAND_SIZE * 2)).min(MAX_PLAYERS)
}

fn create(settings: &RoomSettings) -> Box<dyn Game> {
    let cards = deck::starter_deck();
    Box::new(BadTakes::new(
        settings.points_to_win,
        cards.prompts.clone(),
        cards.answers.clone(),
        rand::make_rng(),
    ))
}

// What players can do, sent as {"kind": "submit", ...}, {"kind": "pull"} and so on
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Action {
    Submit { cards: Vec<CardId> },
    // The judge pulls the next answer out of the bag
    Pull,
    // The judge shows every answer together, once the bag is empty
    ShowAll,
    // The judge points at any card of their favorite answer
    Pick { card: CardId },
}

pub struct BadTakes {
    points_to_win: u8,
    players: Vec<PlayerId>,
    // Everyone who has sat at the table, in join order. Leavers stay, so the judge
    // rotation still knows where they sat
    seating: Vec<PlayerId>,
    // Late joiners, seated at the start of the next round
    waiting: Vec<PlayerId>,
    // Seats at the table, limited by the deck
    max_players: usize,
    // Points per player, in player order so every view lists them the same way
    scores: BTreeMap<PlayerId, u8>,
    prompts: Deck<PromptCard>,
    // Prompts from earlier rounds, shuffled back in when the prompt deck runs out
    used_prompts: Vec<PromptCard>,
    answers: Deck<AnswerCard>,
    // Played cards, out of the game until the answer deck runs out
    discard: Vec<AnswerCard>,
    // Each player's cards, only ever shown to that player
    hands: HashMap<PlayerId, Vec<AnswerCard>>,
    round: u32,
    judge: Option<PlayerId>,
    prompt: Option<PromptCard>,
    // The cards each player put down this round, kept secret until the judge reads them
    submissions: HashMap<PlayerId, Vec<AnswerCard>>,
    // The order the judge reads the cards in, never sent to anyone
    reading_order: Vec<PlayerId>,
    // How many answers the judge has pulled out of the bag, from the start of the reading order
    pulled: usize,
    // How many auto-pull timers have started this round. Only the latest one counts
    pull_timers: TimerId,
    // Has the judge tapped at all this round
    judge_tapped: bool,
    // Has a card come out by itself since the judge last tapped
    judge_missed_tap: bool,
    // Whose cards the judge picked, kept secret until the reveal
    winner: Option<PlayerId>,
    phase: Phase,
    rng: StdRng,
}

impl BadTakes {
    // Tests pass a seeded rng, so the same cards are dealt every run
    pub fn new(
        points_to_win: u8,
        prompts: Vec<PromptCard>,
        answers: Vec<AnswerCard>,
        mut rng: StdRng,
    ) -> BadTakes {
        BadTakes {
            points_to_win,
            players: Vec::new(),
            seating: Vec::new(),
            waiting: Vec::new(),
            max_players: max_players(answers.len()),
            scores: BTreeMap::new(),
            prompts: Deck::shuffled(prompts, &mut rng),
            used_prompts: Vec::new(),
            answers: Deck::shuffled(answers, &mut rng),
            discard: Vec::new(),
            hands: HashMap::new(),
            round: 0,
            judge: None,
            prompt: None,
            submissions: HashMap::new(),
            reading_order: Vec::new(),
            pulled: 0,
            pull_timers: 0,
            judge_tapped: false,
            judge_missed_tap: false,
            winner: None,
            phase: Phase::Submitting,
            rng,
        }
    }

    // Everyone has 7 cards again, then a new judge reads out a new prompt
    fn start_round(&mut self, ctx: &mut GameCtx) {
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

    fn timer_id(&self, timer: Timer) -> TimerId {
        self.round * 100 + timer as TimerId
    }

    // Each tap starts a new auto-pull timer, so each one gets its own id
    fn pull_timer_id(&self) -> TimerId {
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

    // The judge left mid-round: everyone takes their cards back and the next judge starts again
    fn cancel_round(&mut self, ctx: &mut GameCtx) {
        for (player, cards) in self.submissions.drain() {
            if let Some(hand) = self.hands.get_mut(&player) {
                hand.extend(cards);
            }
        }
        ctx.broadcast(json!({ "event": "round_cancelled" }));
        self.start_round(ctx);
    }

    fn submit(
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
    fn judge_when_everyone_submitted(&mut self, ctx: &mut GameCtx) {
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

    // The answers go into a bag, for the judge to pull out one at a time
    fn start_reading(&mut self, ctx: &mut GameCtx) {
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

    fn pull(&mut self, player: PlayerId, ctx: &mut GameCtx) -> Result<(), GameError> {
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

    fn show_all(&mut self, player: PlayerId, ctx: &mut GameCtx) -> Result<(), GameError> {
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
    fn in_bag(&self) -> usize {
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
    fn pull_time_up(&mut self, ctx: &mut GameCtx) {
        self.judge_missed_tap = true;
        if self.in_bag() > 0 {
            self.pull_next(ctx);
        } else {
            self.start_judging(ctx);
        }
    }

    // All the answers are shown together, and the judge's time to pick starts
    fn start_judging(&mut self, ctx: &mut GameCtx) {
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

    // Whoever hasn't played by now sits this round out
    fn submit_time_up(&mut self, ctx: &mut GameCtx) {
        if self.submissions.is_empty() {
            self.skip_round(ctx);
        } else {
            self.start_reading(ctx);
        }
    }

    // Nothing to judge: no point this round, and the next judge starts a new one
    fn skip_round(&mut self, ctx: &mut GameCtx) {
        ctx.broadcast(json!({ "event": "round_skipped" }));
        self.start_round(ctx);
    }

    // The reveal is over: the game ends, or the next round starts
    fn reveal_over(&mut self, ctx: &mut GameCtx) {
        if let Some(winner) = self.game_winner() {
            self.end_game(Some(winner), "points_to_win", ctx);
        } else {
            self.start_round(ctx);
        }
    }

    // The final scores, and why the game ended
    fn end_game(&mut self, winner: Option<PlayerId>, reason: &str, ctx: &mut GameCtx) {
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

    // The winner's answer. It stops being secret once the judge has picked it
    fn winning_cards(&self) -> Option<&Vec<AnswerCard>> {
        self.winner.and_then(|winner| self.submissions.get(&winner))
    }

    fn pick(&mut self, player: PlayerId, card: CardId, ctx: &mut GameCtx) -> Result<(), GameError> {
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
    fn judge_time_up(&mut self, ctx: &mut GameCtx) {
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

    // First to the room's points to win takes the game
    fn game_winner(&self) -> Option<PlayerId> {
        self.scores
            .iter()
            .find(|&(_, &points)| points >= self.points_to_win)
            .map(|(&player, _)| player)
    }

    // Who has played this round, in a fixed order
    fn submitted_players(&self) -> Vec<PlayerId> {
        let mut players: Vec<PlayerId> = self.submissions.keys().copied().collect();
        players.sort();
        players
    }

    // The answers out of the bag so far, in reading order, with nothing to say whose they are
    fn cards_to_judge(&self) -> Vec<&Vec<AnswerCard>> {
        self.reading_order
            .iter()
            .take(self.pulled)
            .filter_map(|player| self.submissions.get(player))
            .collect()
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
}

impl Game for BadTakes {
    fn start(&mut self, players: &[PlayerId], ctx: &mut GameCtx) {
        self.players = players.to_vec();
        self.seating = players.to_vec();
        for &player in players {
            self.scores.insert(player, 0);
        }
        self.start_round(ctx);
    }

    fn handle_action(
        &mut self,
        player: PlayerId,
        action: Value,
        ctx: &mut GameCtx,
    ) -> Result<(), GameError> {
        let Ok(action) = serde_json::from_value::<Action>(action) else {
            return Err(GameError {
                code: "unknown_action",
                message: "That action doesn't exist in this game",
            });
        };
        match action {
            Action::Submit { cards } => self.submit(player, cards, ctx),
            Action::Pull => self.pull(player, ctx),
            Action::ShowAll => self.show_all(player, ctx),
            Action::Pick { card } => self.pick(player, card, ctx),
        }
    }

    fn handle_timer(&mut self, timer: TimerId, ctx: &mut GameCtx) {
        // Each timer only acts if its round is still at the step it was waiting on
        if timer == self.timer_id(Timer::Submit) && self.phase == Phase::Submitting {
            self.submit_time_up(ctx);
        } else if timer == self.pull_timer_id() && self.phase == Phase::Reading {
            self.pull_time_up(ctx);
        } else if timer == self.timer_id(Timer::Judge) && self.phase == Phase::Judging {
            self.judge_time_up(ctx);
        } else if timer == self.timer_id(Timer::Reveal) && self.phase == Phase::RoundOver {
            self.reveal_over(ctx);
        }
    }

    fn player_joined(&mut self, player: PlayerId, _ctx: &mut GameCtx) {
        self.waiting.push(player);
    }

    fn player_left(&mut self, player: PlayerId, ctx: &mut GameCtx) {
        self.players.retain(|&id| id != player);
        self.waiting.retain(|&id| id != player);
        self.scores.remove(&player);
        // Their cards go to the discard pile, so the deck doesn't shrink
        if let Some(hand) = self.hands.remove(&player) {
            self.discard.extend(hand);
        }
        // They aren't here to win the point
        if let Some(cards) = self.submissions.remove(&player) {
            self.discard.extend(cards);
        }
        // If their answer was already out of the bag, one less is out, so the next stays hidden
        if let Some(position) = self.reading_order.iter().position(|&id| id == player) {
            self.reading_order.remove(position);
            if position < self.pulled {
                self.pulled -= 1;
            }
        }

        // Waiting players count, they take a seat at the next round
        let too_few = self.players.len() + self.waiting.len() < MIN_PLAYERS;
        if too_few && self.phase != Phase::GameOver {
            self.end_game(None, "not_enough_players", ctx);
            return;
        }

        let mid_round = matches!(
            self.phase,
            Phase::Submitting | Phase::Reading | Phase::Judging
        );
        let answers_out = matches!(self.phase, Phase::Reading | Phase::Judging);
        if mid_round && self.judge == Some(player) {
            self.cancel_round(ctx);
        } else if answers_out && self.reading_order.is_empty() {
            // The last answer on the table left with its player
            self.skip_round(ctx);
        }
        // The others no longer wait for them
        self.judge_when_everyone_submitted(ctx);
    }

    fn view_for(&self, player: PlayerId) -> Value {
        json!({
            "points_to_win": self.points_to_win,
            "scores": self.scores,
            "waiting": self.waiting,
            "round": self.round,
            "phase": self.phase,
            "judge": self.judge,
            "prompt": self.prompt,
            "hand": self.hands.get(&player),
            "submitted": self.submitted_players(),
            "my_submission": self.submissions.get(&player),
            "cards_to_judge": self.cards_to_judge(),
            "in_bag": self.in_bag(),
            "winner": self.winner,
            "winning_cards": self.winning_cards(),
        })
    }

    fn is_finished(&self) -> bool {
        self.phase == Phase::GameOver
    }
}

// Ids of every answer card in a message, for tests that check hands stay private.
// Answer cards are {"id", "text"} objects, prompts have a "pick" as well, so they're left out
#[cfg(test)]
pub(crate) fn answer_ids(value: &Value, ids: &mut HashSet<u64>) {
    match value {
        Value::Object(fields) => {
            let is_answer = fields.contains_key("text") && !fields.contains_key("pick");
            if let (true, Some(id)) = (is_answer, fields.get("id").and_then(Value::as_u64)) {
                ids.insert(id);
            }
            for field in fields.values() {
                answer_ids(field, ids);
            }
        }
        Value::Array(items) => {
            for item in items {
                answer_ids(item, ids);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests;
