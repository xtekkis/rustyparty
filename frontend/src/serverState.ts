// Everything the server has told this phone, built up one message at a time
import type { ServerMessage } from './protocol'

// The server message of one type, like Of<'lobby'>
type Of<T extends ServerMessage['type']> = Extract<ServerMessage, { type: T }>

export interface ServerState {
  // Who this phone is, once the server has let it in
  welcome: Of<'welcome'> | null
  // The players, the settings and the room's phase
  lobby: Of<'lobby'> | null
  vote: Of<'vote_state'> | null
  voteResult: Of<'vote_result'> | null
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
  vote: null,
  voteResult: null,
  game: null,
  event: null,
  results: null,
  error: null,
  removed: null,
}

export function applyMessage(state: ServerState, message: ServerMessage): ServerState {
  switch (message.type) {
    case 'welcome':
      return { ...state, welcome: message }
    case 'lobby':
      return { ...leftoversCleared(state, message.phase), lobby: message }
    case 'vote_state':
      return { ...state, vote: message }
    case 'vote_result':
      return { ...state, voteResult: message }
    case 'game_view':
      return { ...state, game: message }
    case 'game_event':
      return { ...state, event: { number: (state.event?.number ?? 0) + 1, event: message.event } }
    case 'results':
      return { ...state, results: message }
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
      return { ...state, voteResult: null, game: null, event: null, results: null }
    case 'in_game':
      return { ...state, vote: null, results: null }
    case 'results':
      return { ...state, vote: null }
  }
}
