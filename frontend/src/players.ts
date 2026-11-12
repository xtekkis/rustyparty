// Players from the server, in the shape the design kit draws
import type { LobbyPlayer } from './protocol'
import type { AvatarKind, PlayerView } from './ui'

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
