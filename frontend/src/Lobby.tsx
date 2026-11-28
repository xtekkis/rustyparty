// The waiting room: the room code and everyone in it
import { useEffect, useState } from 'react'
import { toPlayerView } from './players'
import { LobbyScreen } from './ui'
import { useFlash } from './useFlash'
import { useInvite } from './useInvite'
import type { Server } from './useServer'

interface LobbyProps {
  code: string
  server: Server
  // After leaving the room
  onLeft: () => void
}

export function Lobby({ code, server, onLeft }: LobbyProps) {
  const { welcome, lobby, lobbyAt } = server.state
  const [now, setNow] = useState(Date.now)
  const flash = useFlash()
  const invite = useInvite(code, 'platform', flash.show)
  // Leave was tapped, or the back gesture used, and Leave or Stay is showing
  const [confirmingLeave, setConfirmingLeave] = useState(false)

  // The phone's back gesture asks the same as the Leave tile, or closes the invite card if it's
  // open. An extra history entry catches it, and is put back each time so the next back gesture
  // is caught too
  const { isOpen: inviteOpen, close: closeInvite } = invite
  useEffect(() => {
    if (!history.state?.lobby) history.pushState({ lobby: true }, '')
    const back = () => {
      history.pushState({ lobby: true }, '')
      if (inviteOpen) {
        closeInvite()
      } else {
        setConfirmingLeave(true)
      }
    }
    window.addEventListener('popstate', back)
    return () => window.removeEventListener('popstate', back)
  }, [inviteOpen, closeInvite])

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

  return (
    <>
      <LobbyScreen
        code={code}
        qr={invite.qr}
        players={players}
        meId={String(welcome.player_id)}
        ready={ready}
        // The button changes when the server's lobby message says so, not before
        onToggleReady={() => server.send({ type: 'ready', ready: !ready })}
        onCopy={invite.copy}
        onShare={invite.share}
        onShowQr={invite.open}
        // Only the host gets the points setting, and Kick and Ban on everyone else
        pointsToWin={isHost ? lobby.settings.points_to_win : undefined}
        onPointsToWin={(value) => server.send({ type: 'room_settings', points_to_win: value })}
        onKick={(id) => server.send({ type: 'kick', player_id: Number(id) })}
        onBan={(id) => server.send({ type: 'ban', player_id: Number(id) })}
        confirmingLeave={confirmingLeave}
        onLeave={() => setConfirmingLeave(true)}
        onLeaveCancelled={() => setConfirmingLeave(false)}
        onLeaveConfirmed={() => {
          server.send({ type: 'leave' })
          onLeft()
        }}
      />
      {invite.card}
      {flash.banner}
    </>
  )
}
