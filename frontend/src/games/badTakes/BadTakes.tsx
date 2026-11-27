// Bad Takes on a phone: picks the screen from what the game's view says
import { scoreRows, toPlayerView } from '../../players'
import { FinalResultsScreen, LateJoinScreen, Screen, Strip } from '../../ui'
import { useCountdown } from '../../useCountdown'
import { useFlash } from '../../useFlash'
import { useInvite } from '../../useInvite'
import { useNewEvents } from '../../useNewEvents'
import type { Server } from '../../useServer'
import { Judging } from './Judging'
import { Play } from './Play'
import { Reveal } from './Reveal'
import { Winner } from './Winner'
import { Wait } from './Wait'
import type { BadTakesAction, BadTakesEvent, BadTakesView } from './protocol'

interface BadTakesProps {
  code: string
  server: Server
}

export function BadTakes({ code, server }: BadTakesProps) {
  const { game, events, results, gameAt, resultsAt } = server.state
  const seconds = useCountdown(game?.ends_in, gameAt)
  const secondsToLobby = useCountdown(results?.ends_in, resultsAt)
  const flash = useFlash('bad-takes')
  // The game goes on under the invite card, so nobody loses the cards they picked
  const invite = useInvite(code, 'bad-takes', flash.show)

  // Round messages, shown briefly over the next round
  useNewEvents(events, (event) => {
    const happened = (event as BadTakesEvent).event
    if (happened === 'round_skipped') flash.show('Nobody played! New round')
    if (happened === 'round_cancelled') flash.show('The judge is gone. Round cancelled, cards returned')
  })

  return (
    <>
      {screenFor(server, seconds, secondsToLobby, invite.open)}
      {invite.card}
      {flash.banner}
    </>
  )
}

// The screen for this phone, from what the game's view says
function screenFor(server: Server, seconds: number, secondsToLobby: number, onInvite: () => void) {
  const { welcome, lobby, game, events } = server.state
  if (!welcome || !lobby) return null
  if (!game) {
    // Back during the final results: the server has already let the game go
    return <Screen centerGlance glance={<Strip center>The game is over</Strip>} />
  }
  const view = game.view as BadTakesView
  const me = welcome.player_id

  // The game is over: the winner and their winning card, then back to the lobby
  if (view.phase === 'game_over') {
    const champion = lobby.players.find((player) => player.id === view.winner)
    return (
      <FinalResultsScreen
        winner={champion ? toPlayerView(champion, lobby.host) : undefined}
        line={
          view.winning_cards
            ? view.winning_cards.map((card) => card.text).join(' / ')
            : 'Not enough players left'
        }
        rows={scoreRows(view.scores, lobby)}
        secondsToLobby={secondsToLobby}
      />
    )
  }

  // Joined mid-game: seated at the next round, and sees the scores meanwhile
  if (view.waiting.includes(me)) {
    return <LateJoinScreen world="bad-takes" rows={scoreRows(view.scores, lobby)} />
  }

  const send = (action: BadTakesAction) => server.send({ type: 'game_action', action })
  const judgePlayer = lobby.players.find((player) => player.id === view.judge)
  const judge = judgePlayer ? toPlayerView(judgePlayer, lobby.host) : undefined
  // Seated players are the ones with a score, and the judge doesn't play
  const others = lobby.players
    .filter((player) => String(player.id) in view.scores && player.id !== view.judge)
    .map((player) => ({
      ...toPlayerView(player, lobby.host),
      played: view.submitted.includes(player.id),
    }))

  // Your turn to play: a new round's picks start empty, because the key changes
  if (view.phase === 'submitting' && judge && me !== view.judge && !view.my_submission) {
    return (
      <Play key={view.round} view={view} judge={judge} others={others} seconds={seconds} onAction={send} onInvite={onInvite} />
    )
  }

  // The judge for the whole playing time, everyone else once they've played, and all of
  // them until the first card comes out of the bag
  const playing = view.phase === 'submitting' && (me === view.judge || view.my_submission)
  const allIn = view.phase === 'reading' && view.cards_to_judge.length === 0
  if (judge && (playing || allIn)) {
    return (
      <Wait
        key={view.round}
        view={view}
        judge={judge}
        isJudge={me === view.judge}
        others={others}
        seconds={seconds}
        events={events}
        onAction={send}
        onInvite={onInvite}
      />
    )
  }
  // Cards coming out of the bag, until the judge shows them all
  if (judge && view.phase === 'reading') {
    return (
      <Reveal
        key={view.round}
        view={view}
        judge={judge}
        isJudge={me === view.judge}
        events={events}
        onAction={send}
        onInvite={onInvite}
      />
    )
  }

  // Every answer shown together, until the judge gives one the point
  if (judge && view.phase === 'judging') {
    return (
      <Judging
        key={view.round}
        view={view}
        judge={judge}
        isJudge={me === view.judge}
        seconds={seconds}
        onAction={send}
        onInvite={onInvite}
      />
    )
  }

  // The round's winner, with the scores going up
  const winnerPlayer = lobby.players.find((player) => player.id === view.winner)
  if (view.phase === 'round_over' && winnerPlayer) {
    return (
      <Winner
        view={view}
        winner={toPlayerView(winnerPlayer, lobby.host)}
        rows={scoreRows(view.scores, lobby)}
        events={events}
      />
    )
  }

  // Only for a moment, when the judge or the round's winner has just left the room
  return <Screen world="bad-takes" centerGlance glance={<Strip center>Next round coming up</Strip>} />
}
