// What the vote screen needs to know about each game, beyond what the server sends
import type { VoteOption } from './protocol'
import type { World } from './ui'

// Each game's own look on its card, by game id
export const WORLDS: Record<string, World> = { bad_takes: 'bad-takes' }

// Why a game can't be picked with this many players, or undefined when it can
export function unavailableReason(option: VoteOption, players: number): string | undefined {
  if (option.available) return undefined
  if (players < option.min_players) return `needs at least ${option.min_players} players`
  return `takes up to ${option.max_players} players`
}
