// The kit's game cards, built from what the server sends about each game
import { toPlayerView } from './players'
import type { ServerMessage, VoteOption } from './protocol'
import type { GameOption, World } from './ui'

type Of<T extends ServerMessage['type']> = Extract<ServerMessage, { type: T }>

// Each game's own look on its card, by game id
export const WORLDS: Record<string, World> = { bad_takes: 'bad-takes' }

// Why a game can't be picked with this many players, or undefined when it can
export function unavailableReason(option: VoteOption, players: number): string | undefined {
  if (option.available) return undefined
  if (players < option.min_players) return `needs at least ${option.min_players} players`
  return `takes up to ${option.max_players} players`
}

// Every game in a vote, with the avatars of who voted for it
export function gameOptions(vote: Of<'vote_state'>, lobby: Of<'lobby'>): GameOption[] {
  const votersFor = (game: string) =>
    (vote.votes[game] ?? []).flatMap((id) => {
      const player = lobby.players.find((candidate) => candidate.id === id)
      return player ? [toPlayerView(player, lobby.host)] : []
    })

  return vote.options.map((option) => ({
    id: option.id,
    name: option.name,
    description: option.description,
    world: WORLDS[option.id],
    voters: votersFor(option.id),
    // A game that doesn't fit is greyed out, with why
    unavailableReason: unavailableReason(option, lobby.players.length),
  }))
}
