import { useEffect, useState } from 'react'
import { createRoom } from './api'
import { JoinRoom } from './JoinRoom'
import { Profile } from './Profile'
import { saveSession } from './session'
import { HomeScreen, RoomCode, Screen, StatusBanner, Strip } from './ui'
import { useServer } from './useServer'

// How long an error stays on screen
const ERROR_MS = 4000

type Showing = { screen: 'home' } | { screen: 'join' } | { screen: 'profile'; code: string }

// The app's root: picks the screen
function App() {
  const server = useServer()
  const [showing, setShowing] = useState<Showing>({ screen: 'home' })
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    if (!error) return
    const timer = setTimeout(() => setError(null), ERROR_MS)
    return () => clearTimeout(timer)
  }, [error])

  async function create() {
    // A second tap while waiting would make a second room
    if (busy) return
    setBusy(true)
    try {
      setShowing({ screen: 'profile', code: await createRoom() })
    } catch (failure) {
      setError((failure as Error).message)
    } finally {
      setBusy(false)
    }
  }

  const { welcome, lobby } = server.state
  const room = showing.screen === 'profile' ? showing.code : null

  // Remember the seat as soon as the server gives one
  useEffect(() => {
    if (welcome && room) saveSession({ room, token: welcome.token })
  }, [welcome, room])
  const me = lobby?.players.find((player) => player.id === welcome?.player_id)

  let screen
  if (welcome && showing.screen === 'profile') {
    // Until the lobby screen: shows this phone got in
    screen = (
      <Screen
        glance={<><RoomCode code={showing.code} /><Strip center>You're in as {me?.nickname}</Strip></>}
      />
    )
  } else if (showing.screen === 'profile') {
    screen = <Profile code={showing.code} server={server} />
  } else if (showing.screen === 'join') {
    screen = (
      <JoinRoom
        onFound={(code) => setShowing({ screen: 'profile', code })}
        onBack={() => setShowing({ screen: 'home' })}
      />
    )
  } else {
    screen = <HomeScreen onCreate={create} onJoin={() => setShowing({ screen: 'join' })} />
  }

  return (
    <>
      {screen}
      {error ? (
        <div data-theme="platform">
          <StatusBanner tone="alert">{error}</StatusBanner>
        </div>
      ) : null}
    </>
  )
}

export default App
