// Everything the server has told this phone, built up one message at a time
import type { ServerMessage } from './protocol'

// The server message of one type, like Of<'lobby'>
type Of<T extends ServerMessage['type']> = Extract<ServerMessage, { type: T }>

export interface ServerState {
  // Who this phone is, once the server has let it in
  welcome: Of<'welcome'> | null
  // The players, the settings and the room's phase
  lobby: Of<'lobby'> | null
  // When the lobby message arrived, in ms, so reconnect countdowns can keep ticking
  lobbyAt: number
  vote: Of<'vote_state'> | null
  // When the latest vote, game view and results messages arrived, in ms, so countdowns keep ticking
  voteAt: number
  gameAt: number
  resultsAt: number
  // How the last vote ended: its result, or null when nobody voted. The vote is kept with it,
  // because the server clears it the moment the game starts
  voteEnd: { result: Of<'vote_result'> | null; vote: Of<'vote_state'> | null; at: number } | null
  game: Of<'game_view'> | null
  // Each game event gets the next number, so a screen can react to every one, even repeats
  event: { number: number; event: unknown } | null
  results: Of<'results'> | null
  // The last error, for a screen to show briefly
  error: Of<'error'> | null
  // Why the server closed this connection
  removed: 'kicked' | 'banned' | 'replaced' | null
}

export const initialState: ServerState = {
  welcome: null,
  lobby: null,
  lobbyAt: 0,
  vote: null,
  voteAt: 0,
  gameAt: 0,
  resultsAt: 0,
  voteEnd: null,
  game: null,
  event: null,
  results: null,
  error: null,
  removed: null,
}

// at: when the message arrived, passed in so this stays a pure function
export function applyMessage(state: ServerState, message: ServerMessage, at = 0): ServerState {
  switch (message.type) {
    case 'welcome':
      return { ...state, welcome: message }
    case 'lobby': {
      // Straight from voting back to the lobby means nobody voted
      const nobodyVoted = state.lobby?.phase === 'voting' && message.phase === 'lobby'
      const cleared = leftoversCleared(state, message.phase)
      const voteEnd = nobodyVoted ? { result: null, vote: state.vote, at } : cleared.voteEnd
      return { ...cleared, voteEnd, lobby: message, lobbyAt: at }
    }
    case 'vote_state':
      return { ...state, vote: message, voteAt: at }
    case 'vote_result':
      return { ...state, voteEnd: { result: message, vote: state.vote, at } }
    case 'game_view':
      return { ...state, game: message, gameAt: at }
    case 'game_event':
      return { ...state, event: { number: (state.event?.number ?? 0) + 1, event: message.event } }
    case 'results':
      return { ...state, results: message, resultsAt: at }
    case 'error':
      return { ...state, error: message }
    case 'kicked':
    case 'banned':
    case 'replaced':
      return { ...state, removed: message.type }
  }
}

// What the room has moved past. The last game view stays for the results, which show its scores
function leftoversCleared(state: ServerState, phase: Of<'lobby'>['phase']): ServerState {
  switch (phase) {
    case 'lobby':
      return { ...state, vote: null, game: null, event: null, results: null }
    case 'voting':
      return { ...state, voteEnd: null, game: null, event: null, results: null }
    case 'in_game':
      return { ...state, vote: null, results: null }
    case 'results':
      return { ...state, vote: null }
  }
}
