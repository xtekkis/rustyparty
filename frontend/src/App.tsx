import { useEffect, useRef, useState } from 'react'
import { checkRoom, createRoom } from './api'
import { codeFromAddress } from './joinLink'
import { BadTakes } from './games/badTakes/BadTakes'
import { JoinRoom } from './JoinRoom'
import { Lobby } from './Lobby'
import { Profile } from './Profile'
import { Replaced } from './Replaced'
import { Scan } from './Scan'
import { Vote } from './Vote'
import { VoteResult } from './VoteResult'
import { savedToken } from './session'
import { HomeScreen, RemovedScreen, Screen, StatusBanner, Strip, type World } from './ui'
import { useFlash } from './useFlash'
import { useLasting } from './useLasting'
import { useVoteBeat } from './useVoteBeat'
import { useWakeLock } from './useWakeLock'
import { useServer } from './useServer'


// A quick fresh connection, like after unlocking the phone, is done before this and shows nothing
const RECONNECTING_AFTER_MS = 1000

type Showing =
  | { screen: 'home' }
  | { screen: 'join' }
  | { screen: 'scan' }
  | { screen: 'profile'; code: string }

// The app's root: picks the screen
function App() {
  // An invite link opens the profile screen for its room.
  // A phone that already has a seat there goes straight back in instead
  const [invited] = useState(codeFromAddress)
  const [showing, setShowing] = useState<Showing>(() =>
    invited && !savedToken(invited) ? { screen: 'profile', code: invited } : { screen: 'home' },
  )
  const [busy, setBusy] = useState(false)
  const flash = useFlash()
  const showFlash = flash.show
  const server = useServer(invited, (message) => {
    showFlash(message, 'alert')
    setShowing({ screen: 'home' })
  })
  const voteJustEnded = useVoteBeat(server)

  useEffect(() => {
    if (!invited) return
    // Back to a plain address, so a reload or Back doesn't reopen the invite
    history.replaceState(null, '', '/')
    // A saved seat there is checked by asking for it back
    if (savedToken(invited)) return
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

  const { welcome, removed, lobby, error } = server.state

  // The screen stays on from the vote to the final results
  useWakeLock(lobby?.phase === 'voting' || lobby?.phase === 'in_game' || lobby?.phase === 'results')

  // In a room but cut off from the server for more than a moment, and not closed on purpose
  const reconnecting = useLasting(
    server.status !== 'open' && welcome !== null && server.room !== null && removed === null,
    RECONNECTING_AFTER_MS,
  )
  const inGame = lobby?.phase === 'in_game' || lobby?.phase === 'results'
  const world: World = inGame && lobby?.game === 'bad_takes' ? 'bad-takes' : 'platform'

  // Server errors once in a room. Before that, the join screens show them where they belong.
  // Each error is shown once, so an old one never comes back when something else changes
  const lastError = useRef(error)
  useEffect(() => {
    if (error === lastError.current) return
    lastError.current = error
    if (error && welcome) showFlash(error.message, 'alert')
  }, [error, welcome, showFlash])

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
  } else if (removed === 'replaced') {
    screen = <Replaced server={server} onHome={home} />
  } else if (welcome && server.room) {
    // The room's phase picks the screen, after a short beat for the vote's result
    if (voteJustEnded) {
      screen = <VoteResult server={server} />
    } else if (lobby?.phase === 'voting') {
      screen = <Vote server={server} />
    } else if (lobby?.phase === 'in_game' || lobby?.phase === 'results') {
      // The running game's own screens, results included
      screen =
        lobby.game === 'bad_takes' ? (
          <BadTakes server={server} />
        ) : (
          <Screen centerGlance glance={<Strip center>This game isn't in the app yet</Strip>} />
        )
    } else {
      screen = <Lobby code={server.room} server={server} onLeft={home} />
    }
  } else if (server.room) {
    // Asking for the saved seat back, after a reload or reopening the app
    screen = <Screen centerGlance glance={<Strip center>Getting your seat back</Strip>} />
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
      {reconnecting ? (
        // Takes the banner's place at the top, in the colours of the screen below it
        <div data-theme={world}>
          <StatusBanner>Reconnecting…</StatusBanner>
        </div>
      ) : (
        flash.banner
      )}
    </>
  )
}

export default App
