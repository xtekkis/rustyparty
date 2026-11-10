export type AvatarKind = 'fox' | 'cat' | 'owl' | 'bear' | 'frog';
/** A registered world. Each game adds its own theme to tokens.json and its name here. */
export type World = 'platform' | 'bad-takes';
export interface PlayerView {
  id: string;
  nickname: string;
  /** null or missing shows the initials square. */
  avatar?: AvatarKind | null;
  isHost?: boolean;
  ready?: boolean;
  /** false shows the player faded with the reconnect countdown. */
  connected?: boolean;
  reconnectSeconds?: number;
}
export interface ScoreRow { player: PlayerView; score: number; gained?: boolean; }
export interface GameOption {
  id: string;
  name: string;
  description: string;
  /** The game's own world paints its card. */
  world?: World;
  voters: PlayerView[];
  /** Text shown when the game does not fit the player count. Makes the card unselectable. */
  unavailableReason?: string;
}
