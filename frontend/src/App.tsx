import { useEffect, useState } from 'react'
import { createRoom } from './api'
import { JoinRoom } from './JoinRoom'
import { Lobby } from './Lobby'
import { Profile } from './Profile'
import { saveSession } from './session'
import { HomeScreen, RemovedScreen } from './ui'
import { useFlash } from './useFlash'
import { useServer } from './useServer'


type Showing = { screen: 'home' } | { screen: 'join' } | { screen: 'profile'; code: string }

// The app's root: picks the screen
function App() {
  const server = useServer()
  const [showing, setShowing] = useState<Showing>({ screen: 'home' })
  const [busy, setBusy] = useState(false)
  const flash = useFlash()

  async function create() {
    // A second tap while waiting would make a second room
    if (busy) return
    setBusy(true)
    try {
      setShowing({ screen: 'profile', code: await createRoom() })
    } catch (failure) {
      flash.show((failure as Error).message, 'alert')
    } finally {
      setBusy(false)
    }
  }

  const { welcome, removed } = server.state
  const room = showing.screen === 'profile' ? showing.code : null

  // Remember the seat as soon as the server gives one
  useEffect(() => {
    if (welcome && room) saveSession({ room, token: welcome.token })
  }, [welcome, room])
  function home() {
    server.restart()
    setShowing({ screen: 'home' })
  }

  let screen
  if (removed === 'kicked' || removed === 'banned') {
    screen = <RemovedScreen kind={removed} onHome={home} />
  } else if (welcome && showing.screen === 'profile') {
    screen = <Lobby code={showing.code} server={server} />
  } else if (showing.screen === 'profile') {
    screen = <Profile code={showing.code} server={server} onBack={home} />
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
      {flash.banner}
    </>
  )
}

export default App
