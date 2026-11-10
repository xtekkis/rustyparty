import { Avatar, AVATAR_KINDS } from '../primitives/Avatar';
import type { AvatarKind } from '../types';
import styles from './platform.module.css';

export interface NicknameFieldProps { value: string; onChange: (value: string) => void; error?: string; max?: number; }
/** The nickname input, with a visible label and the error right under it. */
export function NicknameField({ value, onChange, error, max = 16 }: NicknameFieldProps) {
  return (
    <div className={styles.field}>
      <label htmlFor="rp-nickname" className={styles.fieldLabel}><span>Nickname</span><span>{[...value].length} of {max}</span></label>
      <input id="rp-nickname" className={styles.input} value={value} maxLength={max} autoComplete="off" autoCorrect="off" enterKeyHint="done" aria-invalid={!!error} aria-describedby={error ? 'rp-nickname-error' : undefined} onChange={(e) => onChange(e.target.value)} />
      {error ? <p id="rp-nickname-error" role="alert" className={styles.fieldError}>{error}</p> : null}
    </div>
  );
}

export interface AvatarPickerProps { nickname: string; value: AvatarKind | null; onChange: (value: AvatarKind | null) => void; }
/** Pick one of the animals, or none for the initials square. */
export function AvatarPicker({ nickname, value, onChange }: AvatarPickerProps) {
  const options: (AvatarKind | null)[] = [...AVATAR_KINDS, null];
  return (
    <div className={styles.picker} role="radiogroup" aria-label="Avatar">
      {options.map((kind) => (
        <button key={kind ?? 'none'} type="button" role="radio" aria-checked={value === kind} aria-label={kind ?? 'Initials'} className={`${styles.pick} ${value === kind ? styles.picked : ''}`} onClick={() => onChange(kind)}>
          <Avatar nickname={nickname || '?'} kind={kind} size="md" />
        </button>
      ))}
    </div>
  );
}
