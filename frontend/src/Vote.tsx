// Everyone picks a game. Voting ends when everyone has voted, or when time runs out
import { unavailableReason, WORLDS } from './games'
import { toPlayerView } from './players'
import { GameVoteScreen, type GameOption } from './ui'
import type { Server } from './useServer'


interface VoteProps {
  server: Server
}

export function Vote({ server }: VoteProps) {
  const { welcome, lobby, vote } = server.state
  if (!welcome || !lobby || !vote) return null

  // Everyone who voted for a game, as avatar bubbles on its card
  const votersFor = (game: string) =>
    (vote.votes[game] ?? []).flatMap((id) => {
      const player = lobby.players.find((candidate) => candidate.id === id)
      return player ? [toPlayerView(player, lobby.host)] : []
    })

  const games: GameOption[] = vote.options.map((option) => ({
    id: option.id,
    name: option.name,
    description: option.description,
    world: WORLDS[option.id],
    voters: votersFor(option.id),
    // A game that doesn't fit is greyed out, with why
    unavailableReason: unavailableReason(option, lobby.players.length),
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
