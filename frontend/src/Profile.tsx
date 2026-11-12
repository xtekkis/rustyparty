// Picking a nickname and an avatar, then asking the server to let this phone in
import { useState } from 'react'
import { UNREACHABLE } from './api'
import { savedToken } from './session'
import type { ServerState } from './serverState'
import { ProfileScreen, type AvatarKind } from './ui'
import type { Server } from './useServer'

interface ProfileProps {
  code: string
  server: Server
}

export function Profile({ code, server }: ProfileProps) {
  const [nickname, setNickname] = useState('')
  const [avatar, setAvatar] = useState<AvatarKind | null>(null)
  // The last error from before tapping Enter, so only a newer one is about this join.
  // undefined until Enter is tapped
  const [errorBefore, setErrorBefore] = useState<ServerState['error'] | undefined>()
  const [offline, setOffline] = useState(false)

  const sent = errorBefore !== undefined
  const answered = sent && server.state.error !== errorBefore
  const waiting = sent && !answered
  const error = offline ? UNREACHABLE : answered ? server.state.error?.message : undefined

  function enter() {
    if (waiting) return
    if (server.status !== 'open') {
      setOffline(true)
      return
    }
    setOffline(false)
    setErrorBefore(server.state.error)
    // With a saved token the server gives back the same seat, and ignores the name and avatar
    const token = savedToken(code)
    server.send({ type: 'join', room: code, nickname: nickname.trim(), avatar, token })
  }

  function typed(value: string) {
    setNickname(value)
    // A new name hides the old error
    setErrorBefore(undefined)
    setOffline(false)
  }

  return (
    <ProfileScreen
      code={code}
      nickname={nickname}
      avatar={avatar}
      error={error}
      onNickname={typed}
      onAvatar={setAvatar}
      onEnter={enter}
    />
  )
}
