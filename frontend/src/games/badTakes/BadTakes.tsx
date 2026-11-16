// Bad Takes on a phone: picks the screen from what the game's view says
import { scoreRows } from '../../players'
import { LateJoinScreen, Screen, Strip } from '../../ui'
import type { Server } from '../../useServer'
import type { BadTakesView } from './protocol'

interface BadTakesProps {
  server: Server
}

export function BadTakes({ server }: BadTakesProps) {
  const { welcome, lobby, game } = server.state
  if (!welcome || !lobby || !game) return null
  const view = game.view as BadTakesView

  // Joined mid-game: seated at the next round, and sees the scores meanwhile
  if (view.waiting.includes(welcome.player_id)) {
    return <LateJoinScreen world="bad-takes" rows={scoreRows(view.scores, lobby)} />
  }
  // Until the round's own screens in the next steps
  return <Screen world="bad-takes" centerGlance glance={<Strip center>The game has started</Strip>} />
}
