// Everyone picks a game. Voting ends when everyone has voted, or when time runs out
import { GameVoteScreen, type GameOption, type World } from './ui'
import type { Server } from './useServer'

// Each game's own look on its card, by game id
const WORLDS: Record<string, World> = { bad_takes: 'bad-takes' }

interface VoteProps {
  server: Server
}

export function Vote({ server }: VoteProps) {
  const { welcome, lobby, vote } = server.state
  if (!welcome || !lobby || !vote) return null

  const games: GameOption[] = vote.options.map((option) => ({
    id: option.id,
    name: option.name,
    description: option.description,
    world: WORLDS[option.id],
    // Voters' avatars and greying out come in the next steps
    voters: [],
  }))
  const myVote = Object.keys(vote.votes).find((game) => vote.votes[game].includes(welcome.player_id))
  const votedCount = Object.values(vote.votes).reduce((count, voters) => count + voters.length, 0)
  // Like the server, only players who are connected need to vote
  const playerCount = lobby.players.filter((player) => player.connected).length

  return (
    <GameVoteScreen
      games={games}
      myVote={myVote}
      seconds={vote.ends_in}
      votedCount={votedCount}
      playerCount={playerCount}
      onVote={(game) => server.send({ type: 'vote', game })}
    />
  )
}
