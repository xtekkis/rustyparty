import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { Icon, type IconName } from './Icon';
import styles from './primitives.module.css';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'success';
  /** A small second line, such as "Tap to undo". */
  sublabel?: string;
  icon?: IconName;
}

/** The large thumb button. One primary per screen. */
export function Button({ variant = 'primary', sublabel, icon, children, className, ...rest }: ButtonProps) {
  return (
    <button type="button" {...rest} className={`${styles.button} ${styles[variant]} ${className ?? ''}`}>
      {icon ? <Icon name={icon} className={styles.buttonIcon} /> : null}
      {children}
      {sublabel ? <small className={styles.sublabel}>{sublabel}</small> : null}
    </button>
  );
}

export interface ActionTileProps extends ButtonHTMLAttributes<HTMLButtonElement> { icon: IconName; label: string; }

/** A small labelled action that sits beside a Button. */
export function ActionTile({ icon, label, className, ...rest }: ActionTileProps) {
  return (
    <button type="button" {...rest} className={`${styles.actionTile} ${className ?? ''}`}>
      <Icon name={icon} /><span>{label}</span>
    </button>
  );
}

export interface StripProps { children: ReactNode; tone?: 'default' | 'alert'; center?: boolean; role?: string; }

/** The solid strip that small text sits on. Small text never sits directly on the dots. */
export function Strip({ children, tone = 'default', center, role }: StripProps) {
  return <p role={role} className={`${styles.strip} ${tone === 'alert' ? styles.alert : ''} ${center ? styles.textCenter : ''}`}>{children}</p>;
}

export interface TopBarProps { children: ReactNode; timer?: ReactNode; }

/** The first row of a screen: a title or the judge, with the timer on the right. */
export function TopBar({ children, timer }: TopBarProps) {
  return <div className={styles.topBar}>{typeof children === 'string' ? <h1 className={styles.title}>{children}</h1> : children}{timer}</div>;
}
