import { useEffect, useRef } from "react";

type AnimationCallback = (t: number) => void;

export function useMascotAnimation(
  active: boolean,
  onFrame: AnimationCallback,
): void {
  const cb = useRef(onFrame);
  cb.current = onFrame;

  useEffect(() => {
    if (!active) return;
    let raf = 0;
    const start = window.performance.now();
    const tick = (now: number) => {
      cb.current((now - start) / 1000);
      raf = window.requestAnimationFrame(tick);
    };
    raf = window.requestAnimationFrame(tick);
    return () => window.cancelAnimationFrame(raf);
  }, [active]);
}