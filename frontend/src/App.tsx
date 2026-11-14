import { useEffect, useState } from 'react'
import { checkRoom, createRoom } from './api'
import { codeFromAddress } from './joinLink'
import { JoinRoom } from './JoinRoom'
import { Lobby } from './Lobby'
import { Profile } from './Profile'
import { Scan } from './Scan'
import { saveSession } from './session'
import { HomeScreen, RemovedScreen } from './ui'
import { useFlash } from './useFlash'
import { useServer } from './useServer'


type Showing =
  | { screen: 'home' }
  | { screen: 'join' }
  | { screen: 'scan' }
  | { screen: 'profile'; code: string }

// The app's root: picks the screen
function App() {
  const server = useServer()
  // An invite link opens the profile screen for its room
  const [invited] = useState(codeFromAddress)
  const [showing, setShowing] = useState<Showing>(
    invited ? { screen: 'profile', code: invited } : { screen: 'home' },
  )
  const [busy, setBusy] = useState(false)
  const flash = useFlash()
  const showFlash = flash.show

  useEffect(() => {
    if (!invited) return
    // Back to a plain address, so a reload or Back doesn't reopen the invite
    history.replaceState(null, '', '/')
    // A room that's gone sends the friend home, before they type a name for nothing
    checkRoom(invited).catch((failure: Error) => {
      showFlash(failure.message, 'alert')
      setShowing({ screen: 'home' })
    })
  }, [invited, showFlash])

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
  // Like an invite link: straight to the profile screen, with the room checked meanwhile
  function scanned(code: string) {
    setShowing({ screen: 'profile', code })
    checkRoom(code).catch((failure: Error) => {
      showFlash(failure.message, 'alert')
      setShowing({ screen: 'join' })
    })
  }

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
        onScan={() => setShowing({ screen: 'scan' })}
        onBack={() => setShowing({ screen: 'home' })}
      />
    )
  } else if (showing.screen === 'scan') {
    screen = (
      <Scan
        onFound={scanned}
        onTypeCode={() => setShowing({ screen: 'join' })}
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
