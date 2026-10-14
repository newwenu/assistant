import {
  Fit,
  Layout,
  useRive,
  useViewModel,
  useViewModelInstance,
  useViewModelInstanceColor,
  useViewModelInstanceEnum,
} from "@rive-app/react-webgl2";
import { type FC, useEffect, useRef } from "react";

import type { MascotRendererProps, RiveRendererConfig } from "../../core/types";
import { asFace } from "../../core/types";
import { resolveVisemeCode, pickIdleFlourish } from "../../core/resolve";
import { MASCOT_STATE_MACHINE, toRiveVisemeCode } from "./riveMaps";
import {
  type RiveStateEngine,
  BUILTIN_RIVE_STATE_ENGINE,
  resolveFaceToPose,
} from "./riveStateEngine";

const AMBIENT_IDLE_MIN_MS = 6_000;
const AMBIENT_IDLE_MAX_MS = 12_000;
const AMBIENT_HOLD_MIN_MS = 2_500;
const AMBIENT_HOLD_MAX_MS = 5_000;

function randBetween(min: number, max: number): number {
  return min + Math.random() * (max - min);
}

const DEFAULT_MASCOT_SRC = "/mascot.riv";

const RIVE_LAYOUT = new Layout({ fit: Fit.Contain });

function useCanvasSizeWatchdog(
  containerRef: React.RefObject<HTMLDivElement | null>,
  rive: ReturnType<typeof useRive>["rive"],
) {
  useEffect(() => {
    if (!rive) return;
    const el = containerRef.current;
    if (!el) return;

    const SETTLE_FRAMES = 5;
    let raf = 0;
    let activeTransitionCount = 0;

    const syncSize = () => {
      const canvas = el.querySelector("canvas");
      if (!canvas) return;
      const layoutW = el.offsetWidth;
      const layoutH = el.offsetHeight;
      if (layoutW === 0 || layoutH === 0) return;

      const dpr = window.devicePixelRatio || 1;
      const expectedW = Math.round(layoutW * dpr);
      const expectedH = Math.round(layoutH * dpr);

      if (canvas.width !== expectedW || canvas.height !== expectedH) {
        canvas.width = expectedW;
        canvas.height = expectedH;
        canvas.style.width = `${layoutW}px`;
        canvas.style.height = `${layoutH}px`;
        if (typeof rive.resizeDrawingSurfaceToCanvas === "function") {
          rive.resizeDrawingSurfaceToCanvas();
        }
      }
    };

    const startPolling = () => {
      if (raf !== 0) return;
      const tick = () => {
        syncSize();
        raf = requestAnimationFrame(tick);
      };
      raf = requestAnimationFrame(tick);
    };

    const stopPolling = () => {
      if (raf !== 0) {
        cancelAnimationFrame(raf);
        raf = 0;
      }
    };

    const runSettling = () => {
      let frame = 0;
      stopPolling();
      const tick = () => {
        syncSize();
        frame++;
        if (frame >= SETTLE_FRAMES) {
          raf = 0;
          if (activeTransitionCount > 0) startPolling();
          return;
        }
        raf = requestAnimationFrame(tick);
      };
      raf = requestAnimationFrame(tick);
    };

    const onTransitionRun = (e: TransitionEvent) => {
      if (e.propertyName === "transform" || e.propertyName === "opacity") {
        activeTransitionCount++;
        startPolling();
      }
    };

    const onTransitionEnd = (e: TransitionEvent) => {
      if (e.propertyName === "transform" || e.propertyName === "opacity") {
        activeTransitionCount = Math.max(0, activeTransitionCount - 1);
        if (activeTransitionCount === 0) {
          syncSize();
          stopPolling();
          runSettling();
        }
      }
    };

    const ancestors: Element[] = [];
    let node: Element | null = el;
    while (node) {
      ancestors.push(node);
      node = node.parentElement;
    }
    for (const a of ancestors) {
      a.addEventListener("transitionrun", onTransitionRun as EventListener);
      a.addEventListener("transitionend", onTransitionEnd as EventListener);
      a.addEventListener("transitioncancel", onTransitionEnd as EventListener);
    }

    runSettling();

    const ro = new ResizeObserver(() => runSettling());
    ro.observe(el);

    let dpiMq: MediaQueryList | null = null;
    const onDpiChange = () => {
      runSettling();
      dpiMq = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
      dpiMq.addEventListener("change", onDpiChange, { once: true });
    };
    dpiMq = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
    dpiMq.addEventListener("change", onDpiChange, { once: true });

    const onVisibilityChange = () => {
      if (!document.hidden) runSettling();
    };
    document.addEventListener("visibilitychange", onVisibilityChange);

    return () => {
      stopPolling();
      ro.disconnect();
      for (const a of ancestors) {
        a.removeEventListener(
          "transitionrun",
          onTransitionRun as EventListener,
        );
        a.removeEventListener(
          "transitionend",
          onTransitionEnd as EventListener,
        );
        a.removeEventListener(
          "transitioncancel",
          onTransitionEnd as EventListener,
        );
      }
      if (dpiMq) dpiMq.removeEventListener("change", onDpiChange);
      document.removeEventListener("visibilitychange", onVisibilityChange);
    };
  }, [rive, containerRef]);
}

export interface RiveMascotProps extends MascotRendererProps {
  stateEngine?: RiveStateEngine;
}

export const RiveMascot: FC<RiveMascotProps> = ({
  face = asFace("idle"),
  size = "100%",
  primaryColor,
  secondaryColor,
  visemeCode = "sil",
  rendererConfig,
  stateEngine,
}) => {
  const config = rendererConfig as RiveRendererConfig | undefined;
  const idlePoseRotation = config?.idlePoseRotation ?? false;
  const src = config?.src ?? DEFAULT_MASCOT_SRC;
  const buffer = config?.buffer ?? undefined;
  const stateMachine = config?.stateMachine ?? MASCOT_STATE_MACHINE;

  const engine: RiveStateEngine = stateEngine ?? BUILTIN_RIVE_STATE_ENGINE;

  const containerRef = useRef<HTMLDivElement>(null);

  const riveSource = buffer ? { buffer } : { src };
  const { rive, RiveComponent } = useRive(
    {
      ...riveSource,
      stateMachines: stateMachine,
      autoplay: true,
      layout: RIVE_LAYOUT,
    },
    {
      shouldResizeCanvasToContainer: true,
    },
  );

  useCanvasSizeWatchdog(containerRef, rive);

  const viewModel = useViewModel(rive, { useDefault: true });
  const vmInstance = useViewModelInstance(viewModel, {
    useDefault: true,
    rive,
  });
  const { setValue: setPose } = useViewModelInstanceEnum("pose", vmInstance);
  const { setValue: setMouthVisemeCode } = useViewModelInstanceEnum(
    "mouthVisemeCode",
    vmInstance,
  );
  const { value: primaryColorValue, setValue: setPrimaryColor } =
    useViewModelInstanceColor("primaryColor", vmInstance);
  const { value: secondaryColorValue, setValue: setSecondaryColor } =
    useViewModelInstanceColor("secondaryColor", vmInstance);

  const defaultPrimaryRef = useRef<number | null>(null);
  const defaultSecondaryRef = useRef<number | null>(null);

  if (defaultPrimaryRef.current === null && primaryColorValue != null) {
    defaultPrimaryRef.current = primaryColorValue;
  }
  if (defaultSecondaryRef.current === null && secondaryColorValue != null) {
    defaultSecondaryRef.current = secondaryColorValue;
  }

  useEffect(() => {
    if (primaryColor != null) {
      setPrimaryColor(primaryColor);
    } else if (defaultPrimaryRef.current != null) {
      setPrimaryColor(defaultPrimaryRef.current);
    }
  }, [primaryColor, setPrimaryColor]);

  useEffect(() => {
    if (secondaryColor != null) {
      setSecondaryColor(secondaryColor);
    } else if (defaultSecondaryRef.current != null) {
      setSecondaryColor(defaultSecondaryRef.current);
    }
  }, [secondaryColor, setSecondaryColor]);

  const basePose = resolveFaceToPose(face, engine);

  useEffect(() => {
    setPose(basePose);
  }, [basePose, setPose]);

  const setPoseRef = useRef(setPose);
  setPoseRef.current = setPose;
  useEffect(() => {
    if (!idlePoseRotation || basePose !== "idle") return;
    let timer: number | undefined;
    let current: string = "idle";
    const toIdle = () => {
      current = "idle";
      setPoseRef.current("idle");
      timer = window.setTimeout(
        toAmbient,
        randBetween(AMBIENT_IDLE_MIN_MS, AMBIENT_IDLE_MAX_MS),
      );
    };
    const toAmbient = () => {
      current = pickIdleFlourish(
        engine.idlePoseCycle,
        current === "idle" ? undefined : current,
      );
      setPoseRef.current(current);
      timer = window.setTimeout(
        toIdle,
        randBetween(AMBIENT_HOLD_MIN_MS, AMBIENT_HOLD_MAX_MS),
      );
    };
    timer = window.setTimeout(
      toAmbient,
      randBetween(AMBIENT_IDLE_MIN_MS, AMBIENT_IDLE_MAX_MS),
    );
    return () => {
      if (timer !== undefined) window.clearTimeout(timer);
      setPoseRef.current("idle");
    };
  }, [idlePoseRotation, basePose, engine]);

  useEffect(() => {
    const riveCode = toRiveVisemeCode(visemeCode);
    const resolved = resolveVisemeCode(riveCode, engine.visemeCodes);
    setMouthVisemeCode(resolved);
  }, [visemeCode, setMouthVisemeCode, engine]);

  const sizeStyle = typeof size === "number" ? `${size}px` : size;

  return (
    <div
      ref={containerRef}
      style={{
        width: sizeStyle,
        height: sizeStyle,
      }}
      data-face={face}
    >
      <RiveComponent
        style={{
          width: sizeStyle,
          height: sizeStyle,
        }}
      />
    </div>
  );
};
