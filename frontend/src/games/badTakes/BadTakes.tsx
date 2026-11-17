// Bad Takes on a phone: picks the screen from what the game's view says
import { scoreRows, toPlayerView } from '../../players'
import { LateJoinScreen, Screen, Strip } from '../../ui'
import type { Server } from '../../useServer'
import { Judging } from './Judging'
import { Play } from './Play'
import { Reveal } from './Reveal'
import { Wait } from './Wait'
import type { BadTakesAction, BadTakesView } from './protocol'

interface BadTakesProps {
  server: Server
}

export function BadTakes({ server }: BadTakesProps) {
  const { welcome, lobby, game, event } = server.state
  if (!welcome || !lobby || !game) return null
  const view = game.view as BadTakesView

  const me = welcome.player_id

  // Joined mid-game: seated at the next round, and sees the scores meanwhile
  if (view.waiting.includes(me)) {
    return <LateJoinScreen world="bad-takes" rows={scoreRows(view.scores, lobby)} />
  }

  const send = (action: BadTakesAction) => server.send({ type: 'game_action', action })
  const seconds = game.ends_in ?? 0
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
      <Play key={view.round} view={view} judge={judge} others={others} seconds={seconds} onAction={send} />
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
        event={event}
        onAction={send}
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
        event={event}
        onAction={send}
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
      />
    )
  }

  // Until the round's own screens in the next steps
  return <Screen world="bad-takes" centerGlance glance={<Strip center>The game has started</Strip>} />
}
