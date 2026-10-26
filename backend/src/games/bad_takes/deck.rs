use serde::{Deserialize, Serialize};

pub type CardId = u32;

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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
