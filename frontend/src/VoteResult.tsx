// A short beat after a vote: which game won, or that nobody voted
import { gameOptions } from './games'
import { VoteResultScreen } from './ui'
import type { Server } from './useServer'

interface VoteResultProps {
  server: Server
}

export function VoteResult({ server }: VoteResultProps) {
  const { voteEnd, lobby } = server.state
  const result = voteEnd?.result
  const winner =
    result && voteEnd.vote && lobby
      ? gameOptions(voteEnd.vote, lobby).find((game) => game.id === result.game)
      : undefined
  // No game shows "Nobody voted. Back to the lobby."
  return <VoteResultScreen game={winner} tie={result?.tie_break} />
}
