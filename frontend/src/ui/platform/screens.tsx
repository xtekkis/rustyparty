import type { ReactNode } from 'react';
import { Screen } from '../primitives/Screen';
import { RoomCode, Wordmark } from '../primitives/Tile';
import { ActionTile, Button, Strip, TopBar } from '../primitives/Controls';
import { Countdown } from '../primitives/Countdown';
import { CodeSlots, LetterTray, ScannerFrame } from './Join';
import { PlayerList, PointsStepper, QrFrame, ReadyRow } from './Lobby';
import { AvatarPicker, NicknameField } from './Profile';
import { GameCard } from './GameCard';
import { Champion, Scoreboard } from './Scoreboard';
import type { AvatarKind, GameOption, PlayerView, ScoreRow, World } from '../types';
import styles from './screens.module.css';

export interface HomeScreenProps { onCreate: () => void; onJoin: () => void; }
export function HomeScreen({ onCreate, onJoin }: HomeScreenProps) {
  return (
    <Screen centerGlance
      glance={<><Wordmark /><Strip center>Party games for the room you're in.</Strip></>}
      thumb={<div className={styles.buttons}><Button onClick={onCreate}>Create room</Button><Button variant="secondary" onClick={onJoin}>Join room</Button></div>} />
  );
}

export interface JoinCodeScreenProps {
  code: string;
  /** Server error, such as "No room with that code". */
  error?: string;
  /** Set while the server checks a full code. */
  checking?: boolean;
  onLetter: (letter: string) => void;
  onDelete: () => void;
  onScan: () => void;
  onBack: () => void;
}
export function JoinCodeScreen({ code, error, checking, onLetter, onDelete, onScan, onBack }: JoinCodeScreenProps) {
  return (
    <Screen
      glance={<><h1 className={styles.joinTitle}>Enter the room code</h1><CodeSlots code={code} />
        <Strip tone={error ? 'alert' : 'default'} role={error ? 'alert' : 'status'}>{error ?? (checking ? 'Looking for that room' : 'Ask the host for the 4 letters.')}</Strip></>}
      thumb={<><LetterTray onLetter={onLetter} onDelete={onDelete} disabled={checking} />
        <div className={styles.backRow}><ActionTile icon="back" label="Back" aria-label="Back to home" onClick={onBack} /><Button variant="secondary" icon="scan" onClick={onScan}>Scan QR code</Button></div></>} />
  );
}

export interface ScanScreenProps {
  /** The live video element. */
  camera?: ReactNode;
  /** True when the camera is blocked or missing. */
  blocked?: boolean;
  onTypeCode: () => void;
  onBack: () => void;
}
export function ScanScreen({ camera, blocked, onTypeCode, onBack }: ScanScreenProps) {
  return (
    <Screen
      glance={<><h1 className={styles.joinTitle}>Scan the QR code</h1><Strip tone={blocked ? 'alert' : 'default'}>{blocked ? 'The camera is off. Type the code instead.' : "Point your camera at the code on the host's phone."}</Strip></>}
      thumb={<><ScannerFrame>{camera}</ScannerFrame><div className={styles.backRow}><ActionTile icon="back" label="Back" aria-label="Back to home" onClick={onBack} /><Button variant="secondary" onClick={onTypeCode}>Type the code</Button></div></>} />
  );
}

export interface ProfileScreenProps {
  code: string;
  nickname: string;
  avatar: AvatarKind | null;
  error?: string;
  onNickname: (value: string) => void;
  onAvatar: (value: AvatarKind | null) => void;
  onEnter: () => void;
}
export function ProfileScreen({ code, nickname, avatar, error, onNickname, onAvatar, onEnter }: ProfileScreenProps) {
  return (
    <Screen
      glance={<><div className={styles.joinHead}><h1 className={styles.joinTitle}>Who's playing?</h1><span className={styles.small}><RoomCode code={code} /></span></div><NicknameField value={nickname} onChange={onNickname} error={error} /></>}
      thumb={<><AvatarPicker nickname={nickname} value={avatar} onChange={onAvatar} /><Button disabled={[...nickname.trim()].length === 0} onClick={onEnter}>Enter room</Button></>} />
  );
}

export interface LobbyScreenProps {
  code: string;
  qr: ReactNode;
  players: PlayerView[];
  meId: string;
  ready: boolean;
  /** Host only. Leave out for other players. */
  pointsToWin?: number;
  onPointsToWin?: (value: number) => void;
  onKick?: (id: string) => void;
  onBan?: (id: string) => void;
  onToggleReady: () => void;
  onCopy: () => void;
  onShare?: () => void;
}
export function LobbyScreen({ code, qr, players, meId, ready, pointsToWin, onPointsToWin, onKick, onBan, onToggleReady, onCopy, onShare }: LobbyScreenProps) {
  const isHost = pointsToWin !== undefined;
  return (
    <Screen
      glance={<><div className={styles.lobbyHead}><RoomCode code={code} /><QrFrame>{qr}</QrFrame></div>{isHost ? <PointsStepper value={pointsToWin} onChange={onPointsToWin ?? (() => {})} /> : null}</>}
      thumb={<><PlayerList players={players} moderatorId={isHost ? meId : undefined} onKick={onKick} onBan={onBan} /><ReadyRow ready={ready} onToggleReady={onToggleReady} onCopy={onCopy} onShare={onShare} /></>} />
  );
}

export interface RemovedScreenProps { kind: 'kicked' | 'banned'; onHome: () => void; }
export function RemovedScreen({ kind, onHome }: RemovedScreenProps) {
  return (
    <Screen centerGlance
      glance={<Strip center tone="default"><span className={styles.bigLine}>{kind === 'banned' ? 'You were banned from this room' : 'You were removed from the room'}</span></Strip>}
      thumb={<Button onClick={onHome}>Back to home</Button>} />
  );
}

export interface GameVoteScreenProps { games: GameOption[]; myVote?: string; seconds: number; votedCount: number; playerCount: number; onVote: (gameId: string) => void; }
export function GameVoteScreen({ games, myVote, seconds, votedCount, playerCount, onVote }: GameVoteScreenProps) {
  // Games people can pick sit closest to the thumb.
  const ordered = [...games].sort((a, b) => Number(!!b.unavailableReason) - Number(!!a.unavailableReason));
  return (
    <Screen
      glance={<><TopBar timer={<Countdown seconds={seconds} />}>Pick a game</TopBar><Strip>{votedCount} of {playerCount} voted</Strip></>}
      thumb={<div className={styles.games}>{ordered.map((g) => <GameCard key={g.id} game={g} selected={g.id === myVote} onSelect={() => onVote(g.id)} />)}</div>} />
  );
}

export interface VoteResultScreenProps { game?: GameOption; tie?: boolean; }
/** Which game won. With no game, nobody voted and the group returns to the lobby. */
export function VoteResultScreen({ game, tie }: VoteResultScreenProps) {
  if (!game) return <Screen centerGlance glance={<Strip center><span className={styles.bigLine}>Nobody voted. Back to the lobby.</span></Strip>} />;
  return (
    <Screen centerGlance
      glance={<><h1 className={styles.resultTitle}>{tie ? 'Tie! Picked at random.' : 'The room picked'}</h1><GameCard game={{ ...game, voters: game.voters }} selected /></>} />
  );
}

export interface LateJoinScreenProps { world?: World; rows: ScoreRow[]; }
export function LateJoinScreen({ world = 'platform', rows }: LateJoinScreenProps) {
  return (
    <Screen world={world}
      glance={<Strip center><span className={styles.bigLine}>You'll join at the next round.</span></Strip>}
      thumb={<Scoreboard rows={rows} />} />
  );
}

export interface FinalResultsScreenProps {
  winner?: PlayerView;
  /** The game's closing line. Bad Takes passes the winning card. Without a winner: "Not enough players left". */
  line: string;
  rows: ScoreRow[];
  secondsToLobby: number;
}
export function FinalResultsScreen({ winner, line, rows, secondsToLobby }: FinalResultsScreenProps) {
  return (
    <Screen
      glance={<><Champion player={winner} title={winner ? `${winner.nickname} wins` : 'Game over'} /><Strip center>{line}</Strip></>}
      thumb={<><Scoreboard rows={rows} /><Strip center><b>Back to the lobby in {secondsToLobby}</b></Strip></>} />
  );
}
