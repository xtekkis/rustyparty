// Players from the server, in the shape the design kit draws
import type { LobbyPlayer, ServerMessage } from './protocol'
import type { AvatarKind, PlayerView, ScoreRow } from './ui'

// secondsGone: how long ago the lobby message arrived, so a reconnect countdown keeps ticking
export function toPlayerView(player: LobbyPlayer, host: number | null, secondsGone = 0): PlayerView {
  return {
    id: String(player.id),
    nickname: player.nickname,
    // The server only accepts the kit's avatar ids
    avatar: player.avatar as AvatarKind | null,
    isHost: player.id === host,
    ready: player.ready,
    connected: player.connected,
    reconnectSeconds:
      player.reconnect_in === null ? undefined : Math.max(0, player.reconnect_in - secondsGone),
  }
}

// A game's scores (player id to points) as scoreboard rows, for players still in the room
export function scoreRows(
  scores: Record<string, number>,
  lobby: Extract<ServerMessage, { type: 'lobby' }>,
): ScoreRow[] {
  return lobby.players.flatMap((player) => {
    const score = scores[String(player.id)]
    return score === undefined ? [] : [{ player: toPlayerView(player, lobby.host), score }]
  })
}
