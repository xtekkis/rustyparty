import { useEffect, useRef } from 'react';
import styles from './primitives.module.css';

/** Dots that grow toward the lower middle of the screen. Redraws at the real size, in the --dots colour of its world. */
export function Halftone({ world }: { world?: string }) {
  const ref = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const draw = () => {
      const { width, height } = canvas.getBoundingClientRect();
      if (!width || !height) return;
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      canvas.width = Math.round(width * dpr);
      canvas.height = Math.round(height * dpr);
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      ctx.scale(dpr, dpr);
      ctx.fillStyle = getComputedStyle(canvas).getPropertyValue('--dots').trim();
      const pitch = 11;
      for (let row = 0; row <= height / pitch + 1; row++) {
        for (let col = 0; col <= width / pitch + 1; col++) {
          const x = col * pitch + (row % 2 ? pitch / 2 : 0);
          const y = row * pitch;
          const dx = (x - width / 2) / (width * 0.78);
          const dy = (y - height * 0.56) / (height * 0.62);
          const strength = Math.max(0, 1 - Math.hypot(dx, dy));
          const radius = Math.pow(strength, 1.3) * 5.25;
          if (radius < 0.45) continue;
          ctx.beginPath();
          ctx.arc(x, y, radius, 0, Math.PI * 2);
          ctx.fill();
        }
      }
    };
    draw();
    const observer = new ResizeObserver(draw);
    observer.observe(canvas);
    return () => observer.disconnect();
  }, [world]);
  return <canvas ref={ref} className={styles.halftone} aria-hidden="true" />;
}
