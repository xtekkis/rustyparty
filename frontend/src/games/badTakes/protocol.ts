// Bad Takes' own messages, matching backend/src/games/bad_takes

export interface PromptCard {
  id: number
  text: string
  // How many answer cards fill it
  pick: 1 | 2
}

export interface AnswerCard {
  id: number
  text: string
}

export type Phase = 'submitting' | 'reading' | 'judging' | 'round_over' | 'game_over'

// What one player sees. Nobody else's hand is ever in it
export interface BadTakesView {
  points_to_win: number
  // Player id to points
  scores: Record<string, number>
  // Late joiners, seated at the next round
  waiting: number[]
  round: number
  phase: Phase
  judge: number | null
  prompt: PromptCard | null
  // null for a late joiner who hasn't been dealt in yet
  hand: AnswerCard[] | null
  // Who has played this round, never what
  submitted: number[]
  my_submission: AnswerCard[] | null
  // The answers out of the bag so far, in reading order, with no names
  cards_to_judge: AnswerCard[][]
  in_bag: number
  winner: number | null
  winning_cards: AnswerCard[] | null
}

export type BadTakesEvent =
  | { event: 'round_started'; round: number; judge: number | null; prompt: PromptCard | null }
  | { event: 'submitted'; player: number }
  | { event: 'reading_started'; answers: number }
  | { event: 'answer_pulled'; cards: AnswerCard[] }
  | { event: 'judging_started' }
  | {
      event: 'winner_revealed'
      winner: number
      cards: AnswerCard[] | null
      points: number
      last_round: boolean
      judge_timed_out: boolean
    }
  | { event: 'round_skipped' }
  | { event: 'round_cancelled' }
  | {
      event: 'game_over'
      reason: 'points_to_win' | 'not_enough_players'
      winner: number | null
      cards: AnswerCard[] | null
      scores: Record<string, number>
    }

// What a player can send, inside a game_action
export type BadTakesAction =
  | { kind: 'submit'; cards: number[] }
  | { kind: 'pull' }
  | { kind: 'show_all' }
  | { kind: 'pick'; card: number }
