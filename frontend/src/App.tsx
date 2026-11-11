import { useEffect, useState } from 'react'
import { createRoom } from './api'
import { JoinRoom } from './JoinRoom'
import { HomeScreen, RoomCode, Screen, StatusBanner, Strip } from './ui'

// How long an error stays on screen
const ERROR_MS = 4000

type Showing = { screen: 'home' } | { screen: 'join' } | { screen: 'room'; code: string }

// The app's root: picks the screen
function App() {
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
      setShowing({ screen: 'room', code: await createRoom() })
    } catch (failure) {
      setError((failure as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <>
      {showing.screen === 'home' ? (
        <HomeScreen onCreate={create} onJoin={() => setShowing({ screen: 'join' })} />
      ) : showing.screen === 'join' ? (
        <JoinRoom
          onFound={(code) => setShowing({ screen: 'room', code })}
          onBack={() => setShowing({ screen: 'home' })}
        />
      ) : (
        // Until the profile screen: shows which room this phone is going into
        <Screen glance={<><RoomCode code={showing.code} /><Strip center>Room ready</Strip></>} />
      )}
      {error ? (
        <div data-theme="platform">
          <StatusBanner tone="alert">{error}</StatusBanner>
        </div>
      ) : null}
    </>
  )
}

export default App
