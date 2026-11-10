// Every message between a phone and the server, matching backend/src/protocol.rs.
// Ids are numbers, and a field the server can leave empty is null

// Messages from the phone to the server
export type ClientMessage =
  | { type: 'join'; room: string; nickname: string; avatar: string | null; token: string | null }
  | { type: 'set_avatar'; avatar: string | null }
  | { type: 'ready'; ready: boolean }
  | { type: 'vote'; game: string }
  | { type: 'room_settings'; points_to_win: number }
  | { type: 'game_action'; action: unknown }
  | { type: 'kick'; player_id: number }
  | { type: 'ban'; player_id: number }
  | { type: 'leave' }

// A player as everyone in the lobby sees them
export interface LobbyPlayer {
  id: number
  nickname: string
  avatar: string | null
  ready: boolean
  connected: boolean
  // Seconds a disconnected player has left to come back
  reconnect_in: number | null
}

// A game players can vote for
export interface VoteOption {
  id: string
  name: string
  description: string
  min_players: number
  max_players: number
  // False when the room has too few or too many players for it
  available: boolean
}

// Room settings the host can change
export interface RoomSettings {
  points_to_win: number
}

// Which part of its cycle a room is in
export type RoomPhase = 'lobby' | 'voting' | 'in_game' | 'results'

// Messages from the server to the phone. A game's view and events are its own,
// so they're unknown here and each game reads them with its own types
export type ServerMessage =
  | { type: 'welcome'; player_id: number; token: string }
  // Sent again every time phase changes, so the latest lobby message says where the room is
  | {
      type: 'lobby'
      players: LobbyPlayer[]
      host: number | null
      settings: RoomSettings
      phase: RoomPhase
    }
  // votes: game id to the ids of the players who voted for it
  | { type: 'vote_state'; options: VoteOption[]; votes: Record<string, number[]>; ends_in: number }
  | { type: 'vote_result'; game: string; tie_break: boolean }
  // ends_in: seconds left on the game's latest timer
  | { type: 'game_view'; view: unknown; ends_in: number | null }
  | { type: 'game_event'; event: unknown }
  | { type: 'results'; ends_in: number }
  | { type: 'error'; code: string; message: string }
  | { type: 'replaced' }
  | { type: 'kicked' }
  | { type: 'banned' }

// POST /rooms
export interface CreateRoomResponse {
  code: string
}

// GET /rooms/{code}
export interface RoomInfo {
  code: string
  players: number
}

// The body of every HTTP error
export interface HttpError {
  code: string
  message: string
}
