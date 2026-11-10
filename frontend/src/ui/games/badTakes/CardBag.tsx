import { forwardRef, useImperativeHandle, useRef, type ReactElement } from 'react';
import styles from './badTakes.module.css';

const BODY = 'M40 42Q100 58 160 42C172 80 186 112 178 140Q172 164 142 164H58Q28 164 22 140C14 112 28 80 40 42Z';

export interface CardBagHandle {
  /** Flies a face-down card from an element into the bag. */
  dropIn: (from?: Element | null) => Promise<void>;
  /** Pulls a card out so it can be read, then flies it to the prompt. Resolves when it lands. */
  pullOut: (text: string, to?: Element | null) => Promise<void>;
  /** Jumps any running animation to its end. Call before the next pull. */
  finish: () => void;
}

const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

/** The bag played cards go into and come out of. It draws no state of its own: call its handle when the server says so. */
export const CardBag = forwardRef<CardBagHandle>(function CardBag(_, ref) {
  const bag = useRef<HTMLDivElement>(null);
  const running = useRef(new Set<Animation>());
  const play = (el: Element, keys: Keyframe[], opts: KeyframeAnimationOptions) => {
    const a = el.animate(keys, opts);
    running.current.add(a);
    const done = () => running.current.delete(a);
    a.onfinish = done;
    a.oncancel = done;
    return a.finished.catch(() => undefined);
  };
  useImperativeHandle(ref, () => ({
    finish: () => { [...running.current].forEach((a) => a.finish()); },
    dropIn: async (from) => {
      const b = bag.current;
      if (!b || reduced()) return;
      const card = document.createElement('span');
      card.className = styles.flying;
      b.append(card);
      const r = from?.getBoundingClientRect();
      const q = card.getBoundingClientRect();
      const dx = r ? r.left + r.width / 2 - q.left - q.width / 2 : 0;
      const dy = r ? r.top + r.height / 2 - q.top - q.height / 2 : -140;
      void play(b, [{ transform: 'scale(1)' }, { transform: 'scale(1.05,.93)' }, { transform: 'scale(1)' }], { duration: 220, delay: 430 });
      await play(card, [
        { transform: `translate(${dx}px,${dy}px) scale(.45) rotate(-14deg)`, easing: 'cubic-bezier(.2,.7,.4,1)' },
        { transform: 'translate(0,-50px) scale(.7) rotate(6deg)', offset: 0.62, easing: 'cubic-bezier(.6,0,.9,.6)' },
        { transform: 'translate(0,12px) scale(.45) rotate(0)' },
      ], { duration: 640 });
      card.remove();
    },
    pullOut: async (text, to) => {
      const b = bag.current;
      if (!b || reduced()) return;
      const card = document.createElement('div');
      card.className = styles.rising;
      card.textContent = text;
      b.append(card);
      const r = to?.getBoundingClientRect();
      const q = card.getBoundingClientRect();
      const dx = r ? r.left + r.width / 2 - q.left - q.width / 2 : 0;
      const dy = r ? r.top + r.height / 2 - q.top - q.height / 2 : -220;
      void play(b, [{ transform: 'rotate(0)' }, { transform: 'rotate(-4deg)' }, { transform: 'rotate(4deg)' }, { transform: 'rotate(0)' }], { duration: 260 });
      await play(card, [
        { transform: 'translateY(8px) scale(.3)', opacity: 1, offset: 0, easing: 'cubic-bezier(.3,.5,.6,1)' },
        { transform: 'translateY(-72px) scale(.42)', opacity: 1, offset: 0.13, easing: 'cubic-bezier(.2,.8,.3,1)' },
        { transform: 'translateY(-126px) scale(1) rotate(-2deg)', opacity: 1, offset: 0.27 },
        { transform: 'translateY(-126px) scale(1) rotate(-2deg)', opacity: 1, offset: 0.78, easing: 'cubic-bezier(.6,0,.9,.5)' },
        { transform: `translate(${dx}px,${dy}px) scale(.5)`, opacity: 0.2, offset: 1 },
      ], { duration: 1900 });
      card.remove();
    },
  }));
  return (
    <div className={styles.bagWrap}>
      <div ref={bag} className={styles.bag} role="img" aria-label="Bag of played cards">
        <svg className={styles.bagBack} viewBox="0 0 200 170" aria-hidden="true"><path d={BODY} fill="var(--bag-shadow)" transform="translate(6 6)" /><ellipse cx="100" cy="38" rx="62" ry="15" fill="var(--bag-mouth)" /></svg>
        <svg className={styles.bagFront} viewBox="0 0 200 170" aria-hidden="true">
          <path d={BODY} fill="var(--bag)" />
          <g fill="var(--bag-dots)">{DOTS}</g>
          <path d="M38 52Q100 70 164 52" fill="none" stroke="var(--bag-mouth)" strokeWidth="5" />
          <path d="M100 62l-11 22M100 62l11 22" fill="none" stroke="var(--bag-mouth)" strokeWidth="4" />
        </svg>
      </div>
    </div>
  );
});

const DOTS = (() => {
  const dots: ReactElement[] = [];
  for (let y = 76; y < 158; y += 11) {
    for (let x = 34; x < 170; x += 11) {
      const px = x + ((y - 76) % 22 ? 5.5 : 0);
      const half = 56 + (y - 76) * 0.42;
      if (Math.abs(px - 100) < half) dots.push(<circle key={`${px}-${y}`} cx={px} cy={y} r="2.2" />);
    }
  }
  return dots;
})();
