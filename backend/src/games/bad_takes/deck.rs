use std::collections::HashSet;
use std::sync::LazyLock;

use rand::Rng;
use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

pub type CardId = u32;

// Built into the program, so the server doesn't depend on the folder it's started from
const STARTER_JSON: &str = include_str!("../../../data/bad_takes_starter.json");

static STARTER: LazyLock<DeckFile> = LazyLock::new(|| {
    parse_deck(STARTER_JSON)
        .unwrap_or_else(|error| panic!("bad_takes_starter.json is invalid: {error}"))
});

pub fn starter_deck() -> &'static DeckFile {
    &STARTER
}

// A card with one or two gaps, read out by the judge
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PromptCard {
    pub id: CardId,
    pub text: String,
    // How many answer cards fill it, 1 or 2
    pub pick: u8,
}

// A card players put in the gap
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AnswerCard {
    pub id: CardId,
    pub text: String,
}

// A pile of cards in random order, drawn from the top
pub struct Deck<T> {
    cards: Vec<T>,
}

impl<T> Deck<T> {
    pub fn shuffled(mut cards: Vec<T>, rng: &mut impl Rng) -> Deck<T> {
        cards.shuffle(rng);
        Deck { cards }
    }

    // The top card, none when the deck is empty
    pub fn draw(&mut self) -> Option<T> {
        self.cards.pop()
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }
}

// All the cards in one deck file
#[derive(Debug, Deserialize)]
pub struct DeckFile {
    #[serde(rename = "deck")]
    pub name: String,
    pub prompts: Vec<PromptCard>,
    pub answers: Vec<AnswerCard>,
}

// Read a deck file and check the cards make sense
pub fn parse_deck(json: &str) -> Result<DeckFile, String> {
    let deck: DeckFile = serde_json::from_str(json).map_err(|error| error.to_string())?;

    if deck.prompts.is_empty() || deck.answers.is_empty() {
        return Err("a deck needs at least one prompt and one answer".to_string());
    }
    for prompt in &deck.prompts {
        if !(1..=2).contains(&prompt.pick) {
            return Err(format!(
                "prompt {} has pick {}, it must be 1 or 2",
                prompt.id, prompt.pick
            ));
        }
        let gaps = prompt.text.matches("____").count();
        if gaps != usize::from(prompt.pick) {
            return Err(format!(
                "prompt {} has {gaps} gaps but pick {}",
                prompt.id, prompt.pick
            ));
        }
    }

    let texts = deck.prompts.iter().map(|card| (card.id, &card.text));
    let texts = texts.chain(deck.answers.iter().map(|card| (card.id, &card.text)));
    for (id, text) in texts {
        if text.trim().is_empty() {
            return Err(format!("card {id} has no text"));
        }
    }

    if let Some(id) = first_duplicate(deck.prompts.iter().map(|card| card.id)) {
        return Err(format!("two prompts have id {id}"));
    }
    if let Some(id) = first_duplicate(deck.answers.iter().map(|card| card.id)) {
        return Err(format!("two answers have id {id}"));
    }
    Ok(deck)
}

fn first_duplicate(ids: impl Iterator<Item = CardId>) -> Option<CardId> {
    let mut seen = HashSet::new();
    ids.into_iter().find(|&id| !seen.insert(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use serde_json::json;

    // The ids of the starter answers in the order a deck seeded like this deals them
    fn dealt_ids(seed: u64) -> Vec<CardId> {
        let answers = starter_deck().answers.clone();
        let mut deck = Deck::shuffled(answers, &mut StdRng::seed_from_u64(seed));
        std::iter::from_fn(|| deck.draw())
            .map(|card| card.id)
            .collect()
    }

    #[test]
    fn same_seed_deals_the_same_order() {
        assert_eq!(dealt_ids(42), dealt_ids(42));
    }

    #[test]
    fn different_seeds_deal_different_orders() {
        assert_ne!(dealt_ids(1), dealt_ids(2));
    }

    #[test]
    fn shuffling_changes_the_order() {
        let file_order: Vec<CardId> = starter_deck().answers.iter().map(|card| card.id).collect();
        let mut dealt = dealt_ids(7);
        assert_ne!(dealt, file_order);

        // Every card is still there exactly once
        dealt.sort();
        assert_eq!(dealt, file_order);
    }

    #[test]
    fn drawing_takes_every_card_once_then_none() {
        let mut deck = Deck::shuffled(vec![1, 2, 3], &mut rand::rng());
        assert_eq!(deck.len(), 3);

        let mut drawn = vec![
            deck.draw().unwrap(),
            deck.draw().unwrap(),
            deck.draw().unwrap(),
        ];
        assert!(deck.is_empty());
        assert_eq!(deck.draw(), None);

        drawn.sort();
        assert_eq!(drawn, [1, 2, 3]);
    }

    #[test]
    fn starter_deck_loads() {
        let deck = starter_deck();
        assert_eq!(deck.name, "Starter");
        assert_eq!(deck.prompts.len(), 45);
        assert_eq!(deck.answers.len(), 140);
    }

    // A small valid deck as JSON text, with one prompt changed by the test
    fn deck_with_prompt(prompt: serde_json::Value) -> String {
        json!({
            "deck": "Test",
            "prompts": [prompt],
            "answers": [{ "id": 1, "text": "A wet sock." }]
        })
        .to_string()
    }

    #[test]
    fn pick_must_be_1_or_2() {
        let json = deck_with_prompt(json!({ "id": 1, "text": "____ ____ ____", "pick": 3 }));
        assert_eq!(
            parse_deck(&json).unwrap_err(),
            "prompt 1 has pick 3, it must be 1 or 2"
        );
    }

    #[test]
    fn gaps_must_match_pick() {
        let json = deck_with_prompt(json!({ "id": 4, "text": "Only ____ here.", "pick": 2 }));
        assert_eq!(
            parse_deck(&json).unwrap_err(),
            "prompt 4 has 1 gaps but pick 2"
        );
    }

    #[test]
    fn empty_cards_are_refused() {
        let json = deck_with_prompt(json!({ "id": 2, "text": "   ", "pick": 1 }));
        // An empty prompt also has no gap, which is caught first
        assert!(parse_deck(&json).is_err());

        let json = json!({
            "deck": "Test",
            "prompts": [{ "id": 1, "text": "Love is ____.", "pick": 1 }],
            "answers": [{ "id": 5, "text": "" }]
        })
        .to_string();
        assert_eq!(parse_deck(&json).unwrap_err(), "card 5 has no text");
    }

    #[test]
    fn duplicate_ids_are_refused() {
        let json = json!({
            "deck": "Test",
            "prompts": [{ "id": 1, "text": "Love is ____.", "pick": 1 }],
            "answers": [{ "id": 3, "text": "A wet sock." }, { "id": 3, "text": "Your mum." }]
        })
        .to_string();
        assert_eq!(parse_deck(&json).unwrap_err(), "two answers have id 3");
    }

    #[test]
    fn broken_json_is_refused() {
        assert!(parse_deck("{ not json").is_err());
    }

    #[test]
    fn prompt_reads_from_the_deck_file_format() {
        let prompt: PromptCard = serde_json::from_value(json!({
            "id": 1,
            "text": "My therapist said I need to stop ____.",
            "pick": 1
        }))
        .unwrap();

        assert_eq!(
            prompt,
            PromptCard {
                id: 1,
                text: "My therapist said I need to stop ____.".to_string(),
                pick: 1,
            }
        );
    }

    #[test]
    fn answer_reads_from_the_deck_file_format() {
        let answer: AnswerCard =
            serde_json::from_value(json!({ "id": 1, "text": "A suspicious rash." })).unwrap();
        assert_eq!(answer.text, "A suspicious rash.");
    }

    #[test]
    fn cards_turn_back_into_the_same_json() {
        let answer = AnswerCard {
            id: 7,
            text: "The sex noises from next door.".to_string(),
        };
        assert_eq!(
            serde_json::to_value(&answer).unwrap(),
            json!({ "id": 7, "text": "The sex noises from next door." })
        );
    }
}
