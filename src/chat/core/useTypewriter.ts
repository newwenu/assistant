import { useState, useEffect, useRef } from "react";

const TYPEWRITER_BASE_CHARS_PER_SEC = 90;
const TYPEWRITER_ACCEL_PER_BACKLOG = 2;
const TYPEWRITER_MAX_CHARS_PER_SEC = 720;
const TYPEWRITER_FINISH_CHARS_PER_SEC = 2400;
const TYPEWRITER_MAX_CHARS_PER_PAINT = 18;
const TYPEWRITER_FINISH_MAX_CHARS_PER_PAINT = 64;
const TYPEWRITER_MIN_PAINT_INTERVAL_MS = 16;
const TYPEWRITER_FINISH_MIN_PAINT_INTERVAL_MS = 8;

interface GraphemeSegment {
  index: number;
  segment: string;
}

interface GraphemeSegmenter {
  segment: (text: string) => Iterable<GraphemeSegment>;
}

type GraphemeSegmenterConstructor = new (
  locale?: string | string[],
  options?: { granularity: "grapheme" },
) => GraphemeSegmenter;

let cachedSegmenter: GraphemeSegmenter | null | undefined;

function getGraphemeSegmenter(): GraphemeSegmenter | null {
  if (cachedSegmenter !== undefined) return cachedSegmenter;
  const Segmenter = (
    Intl as typeof Intl & {
      Segmenter?: GraphemeSegmenterConstructor;
    }
  ).Segmenter;
  cachedSegmenter = Segmenter
    ? new Segmenter(undefined, { granularity: "grapheme" })
    : null;
  return cachedSegmenter;
}

export function safeGraphemeRevealEnd(
  text: string,
  requestedEnd: number,
  knownBoundary = 0,
): number {
  const clamped = Math.max(0, Math.min(text.length, Math.floor(requestedEnd)));
  if (clamped === 0 || clamped === text.length) return clamped;
  const segStart = Math.max(0, Math.min(clamped, Math.floor(knownBoundary)));
  const remaining = text.slice(segStart);
  const segmenter = getGraphemeSegmenter();
  if (segmenter) {
    for (const part of segmenter.segment(remaining)) {
      const end = segStart + part.index + part.segment.length;
      if (end >= clamped) return end;
    }
    return text.length;
  }
  let cpEnd = segStart;
  for (const cp of Array.from(remaining)) {
    cpEnd += cp.length;
    if (cpEnd >= clamped) return cpEnd;
  }
  return text.length;
}

function getPrefersReducedMotion(): boolean {
  return (
    typeof window !== "undefined" &&
    typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
}

function computeCharsPerSec(backlog: number, finishing: boolean): number {
  if (finishing) return TYPEWRITER_FINISH_CHARS_PER_SEC;
  return Math.min(
    TYPEWRITER_BASE_CHARS_PER_SEC +
      Math.max(0, backlog) * TYPEWRITER_ACCEL_PER_BACKLOG,
    TYPEWRITER_MAX_CHARS_PER_SEC,
  );
}

function commitReveal(
  backlog: number,
  carry: number,
  maxPerPaint: number,
): { chars: number; carry: number } {
  const b = Math.max(0, Math.floor(backlog));
  const max = Math.max(1, maxPerPaint);
  let fc = Number.isFinite(carry) ? Math.max(0, carry) : 0;
  if (b <= 0) return { chars: 0, carry: 0 };
  const chars = Math.min(b, Math.floor(fc), max);
  fc = Math.max(0, fc - chars);
  return { chars, carry: fc };
}

export interface TypewriterOptions {
  replayOnMount?: boolean;
}

export interface TypewriterResult {
  displayText: string;
  isRevealing: boolean;
}

export function useTypewriter(
  targetText: string,
  animate: boolean,
  options: TypewriterOptions = {},
): TypewriterResult {
  const replayOnMount = options.replayOnMount ?? false;
  const [prefersReducedMotion, setPrefersReducedMotion] = useState(
    getPrefersReducedMotion,
  );
  const shouldReplay = animate && replayOnMount && !prefersReducedMotion;
  const [displayText, setDisplayText] = useState(
    shouldReplay ? "" : targetText,
  );
  const revealedRef = useRef(shouldReplay ? 0 : targetText.length);
  const targetRef = useRef(targetText);
  const animateRef = useRef(animate);
  const rafRef = useRef<number | null>(null);
  const lastTickMsRef = useRef<number | null>(null);
  const lastPaintMsRef = useRef(0);
  const carryRef = useRef(0);

  const isRevealing =
    !prefersReducedMotion &&
    (animate || displayText.length < targetText.length);

  useEffect(() => {
    animateRef.current = animate;
    targetRef.current = targetText;

    if (prefersReducedMotion) {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
      lastTickMsRef.current = null;
      lastPaintMsRef.current = 0;
      carryRef.current = 0;
      revealedRef.current = targetText.length;
      setDisplayText(targetText);
      return;
    }

    if (targetText.length < revealedRef.current) {
      revealedRef.current = 0;
      carryRef.current = 0;
      lastPaintMsRef.current = 0;
      setDisplayText("");
    }

    if (!animate && revealedRef.current >= targetText.length) {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
      lastTickMsRef.current = null;
      carryRef.current = 0;
      revealedRef.current = targetText.length;
      setDisplayText(targetText);
      return;
    }

    const tick = (nowMs: number) => {
      rafRef.current = null;
      const target = targetRef.current;
      let revealed = revealedRef.current;

      if (target.length < revealed) {
        revealed = 0;
        revealedRef.current = 0;
        carryRef.current = 0;
      }

      const backlog = target.length - revealed;
      if (backlog <= 0) {
        lastTickMsRef.current = null;
        carryRef.current = 0;
        if (revealedRef.current !== target.length) {
          revealedRef.current = target.length;
          setDisplayText(target);
        }
        return;
      }

      const lastTickMs = lastTickMsRef.current;
      lastTickMsRef.current = nowMs;
      const finishing = !animateRef.current;
      const minInterval = finishing
        ? TYPEWRITER_FINISH_MIN_PAINT_INTERVAL_MS
        : TYPEWRITER_MIN_PAINT_INTERVAL_MS;
      const maxPerPaint = finishing
        ? TYPEWRITER_FINISH_MAX_CHARS_PER_PAINT
        : TYPEWRITER_MAX_CHARS_PER_PAINT;
      const dtMs =
        lastTickMs === null
          ? minInterval
          : Math.min(100, Math.max(0, nowMs - lastTickMs));

      const charsPerSec = computeCharsPerSec(backlog, finishing);
      carryRef.current += (charsPerSec * dtMs) / 1000;
      carryRef.current = Math.min(carryRef.current, backlog + maxPerPaint);

      const sincePaint = nowMs - lastPaintMsRef.current;
      const canPaint =
        lastPaintMsRef.current === 0 || sincePaint >= minInterval;

      if (canPaint) {
        const committed = commitReveal(backlog, carryRef.current, maxPerPaint);
        carryRef.current = committed.carry;
        if (committed.chars > 0) {
          const next = safeGraphemeRevealEnd(
            target,
            revealed + committed.chars,
            revealed,
          );
          revealedRef.current = next;
          lastPaintMsRef.current = nowMs;
          setDisplayText(target.slice(0, next));
        }
      }

      if (revealedRef.current < targetRef.current.length) {
        rafRef.current = requestAnimationFrame(tick);
      } else {
        lastTickMsRef.current = null;
      }
    };

    if (rafRef.current === null && targetText.length > revealedRef.current) {
      rafRef.current = requestAnimationFrame(tick);
    }
  }, [targetText, animate, prefersReducedMotion]);

  useEffect(() => {
    if (
      typeof window === "undefined" ||
      typeof window.matchMedia !== "function"
    )
      return;
    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const handleChange = (event: MediaQueryListEvent) =>
      setPrefersReducedMotion(event.matches);
    setPrefersReducedMotion(media.matches);
    media.addEventListener?.("change", handleChange);
    return () => media.removeEventListener?.("change", handleChange);
  }, []);

  useEffect(() => {
    if (typeof document === "undefined") return;
    const revealLatest = () => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
      const target = targetRef.current;
      lastTickMsRef.current = null;
      lastPaintMsRef.current = 0;
      carryRef.current = 0;
      revealedRef.current = target.length;
      setDisplayText(target);
    };
    document.addEventListener("visibilitychange", revealLatest);
    return () => document.removeEventListener("visibilitychange", revealLatest);
  }, []);

  useEffect(() => {
    return () => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
    };
  }, []);

  return { displayText, isRevealing };
}
