// Bad Takes: an adult fill-in-the-blank card game
// The cards, and the starter deck loaded from JSON
pub mod deck;
// Picking the winner and scoring the point
mod judging;
// The bag: the judge pulls the answers out one at a time, then shows them all
mod reading;
// Dealing, the next judge, skipped and cancelled rounds, and the end of the game
mod round;
// Which step of the round the game is at
pub mod state;
// Playing cards
mod submitting;

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use rand::rngs::StdRng;
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

    fn timer_id(&self, timer: Timer) -> TimerId {
        self.round * 100 + timer as TimerId
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
pub(crate) fn answer_ids(value: &Value, ids: &mut std::collections::HashSet<u64>) {
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
