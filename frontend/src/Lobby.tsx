// The waiting room: the room code and everyone in it
import { QRCodeSVG } from 'qrcode.react'
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
  const ready = me?.ready ?? false
  const isHost = lobby.host === welcome.player_id
  // Where an invite takes a friend: straight to the profile screen for this room
  const joinLink = `${location.origin}/join/${code}`

  return (
    <LobbyScreen
      code={code}
      // Copy comes in the next step
      qr={<QRCodeSVG value={joinLink} title={`Join link ${joinLink}`} />}
      players={players}
      meId={String(welcome.player_id)}
      ready={ready}
      // The button changes when the server's lobby message says so, not before
      onToggleReady={() => server.send({ type: 'ready', ready: !ready })}
      onCopy={() => {}}
      // Only the host gets the points setting, and Kick and Ban on everyone else
      pointsToWin={isHost ? lobby.settings.points_to_win : undefined}
      onPointsToWin={(value) => server.send({ type: 'room_settings', points_to_win: value })}
      onKick={(id) => server.send({ type: 'kick', player_id: Number(id) })}
      onBan={(id) => server.send({ type: 'ban', player_id: Number(id) })}
    />
  )
}
