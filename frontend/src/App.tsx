import { useEffect, useState } from 'react'
import { createRoom } from './api'
import { HomeScreen, RoomCode, Screen, StatusBanner, Strip } from './ui'

// How long an error stays on screen
const ERROR_MS = 4000

// The app's root: picks the screen
function App() {
  const [room, setRoom] = useState<string | null>(null)
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
      setRoom(await createRoom())
    } catch (failure) {
      setError((failure as Error).message)
    } finally {
      setBusy(false)
    }
  }

  return (
    <>
      {room ? (
        // Until the profile screen: shows the room was made
        <Screen glance={<><RoomCode code={room} /><Strip center>Room created</Strip></>} />
      ) : (
        // Join room is wired up in the next step
        <HomeScreen onCreate={create} onJoin={() => {}} />
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
