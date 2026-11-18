// Everyone picks a game. Voting ends when everyone has voted, or when time runs out
import { gameOptions } from './games'
import { GameVoteScreen } from './ui'
import { useCountdown } from './useCountdown'
import type { Server } from './useServer'

interface VoteProps {
  server: Server
}

export function Vote({ server }: VoteProps) {
  const { welcome, lobby, vote, voteAt } = server.state
  const seconds = useCountdown(vote?.ends_in, voteAt)
  if (!welcome || !lobby || !vote) return null

  const myVote = Object.keys(vote.votes).find((game) => vote.votes[game].includes(welcome.player_id))
  const votedCount = Object.values(vote.votes).reduce((count, voters) => count + voters.length, 0)
  // Like the server, only players who are connected need to vote
  const playerCount = lobby.players.filter((player) => player.connected).length

  return (
    <GameVoteScreen
      games={gameOptions(vote, lobby)}
      myVote={myVote}
      seconds={seconds}
      votedCount={votedCount}
      playerCount={playerCount}
      onVote={(game) => server.send({ type: 'vote', game })}
    />
  )
}
