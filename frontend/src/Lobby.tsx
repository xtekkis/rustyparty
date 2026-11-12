// The waiting room: the room code and everyone in it
import { useEffect, useState } from 'react'
import { toPlayerView } from './players'
import { LobbyScreen } from './ui'
import type { Server } from './useServer'

interface LobbyProps {
  code: string
  server: Server
}

export function Lobby({ code, server }: LobbyProps) {
  const { welcome, lobby, lobbyAt } = server.state
  const [now, setNow] = useState(Date.now)

  // Tick once a second while someone is away, so their countdown goes down
  const anyoneAway = lobby?.players.some((player) => !player.connected) ?? false
  useEffect(() => {
    if (!anyoneAway) return
    const timer = setInterval(() => setNow(Date.now()), 1000)
    return () => clearInterval(timer)
  }, [anyoneAway])

  if (!welcome || !lobby) return null
  // Reconnect countdowns start from the latest lobby message
  const secondsGone = Math.max(0, Math.floor((now - lobbyAt) / 1000))
  const players = lobby.players.map((player) => toPlayerView(player, lobby.host, secondsGone))
  const me = lobby.players.find((player) => player.id === welcome.player_id)

  return (
    <LobbyScreen
      code={code}
      // The QR code, Ready, Copy and the host's settings come in the next steps
      qr={null}
      players={players}
      meId={String(welcome.player_id)}
      ready={me?.ready ?? false}
      onToggleReady={() => {}}
      onCopy={() => {}}
    />
  )
}
