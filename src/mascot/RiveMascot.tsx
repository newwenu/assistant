import {
  Fit,
  Layout,
  useRive,
  useViewModel,
  useViewModelInstance,
  useViewModelInstanceColor,
  useViewModelInstanceEnum,
} from "@rive-app/react-webgl2";
import { type FC, useCallback, useEffect, useRef, useState } from "react";

import type { MascotFace } from "./faceStates";
import {
  faceToPose,
  MASCOT_STATE_MACHINE,
  pickAmbientPose,
  toRiveVisemeCode,
  type RivePose,
} from "./riveMaps";
import { useOrbStore } from "../stores/orbStore";

const AMBIENT_IDLE_MIN_MS = 6_000;
const AMBIENT_IDLE_MAX_MS = 12_000;
const AMBIENT_HOLD_MIN_MS = 2_500;
const AMBIENT_HOLD_MAX_MS = 5_000;

function randBetween(min: number, max: number): number {
  return min + Math.random() * (max - min);
}

const DEFAULT_MASCOT_SRC = "/mascot.riv";

interface RiveMascotProps {
  face?: MascotFace;
  size?: number | string;
  primaryColor?: number;
  secondaryColor?: number;
  visemeCode?: string;
  idlePoseRotation?: boolean;
  src?: string;
  buffer?: ArrayBuffer;
  stateMachine?: string;
}

const RIVE_LAYOUT = new Layout({ fit: Fit.Contain });

function useFixCanvasUnderTransform(
  containerRef: React.RefObject<HTMLDivElement | null>,
  rive: ReturnType<typeof useRive>["rive"],
) {
  const isExpanded = useOrbStore((s) => s.isExpanded);
  const [canvasReady, setCanvasReady] = useState(false);

  const fixCanvasSize = useCallback(() => {
    const el = containerRef.current;
    if (!el) return;
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
    }

    if (typeof rive?.resizeDrawingSurfaceToCanvas === "function") {
      rive.resizeDrawingSurfaceToCanvas();
    }
  }, [containerRef, rive]);

  useEffect(() => {
    if (!rive || !containerRef.current) return;
    setCanvasReady(true);
    fixCanvasSize();

    const ro = new ResizeObserver(fixCanvasSize);
    ro.observe(containerRef.current);

    return () => ro.disconnect();
  }, [rive, containerRef, fixCanvasSize]);

  useEffect(() => {
    if (!canvasReady || !containerRef.current) return;

    const panel = document.querySelector<HTMLElement>(".orb-panel");
    if (!panel) return;

    const onTransitionEnd = (e: TransitionEvent) => {
      if (e.target === panel && e.propertyName === "transform") {
        fixCanvasSize();
      }
    };

    panel.addEventListener("transitionend", onTransitionEnd as EventListener);

    const timeout = window.setTimeout(fixCanvasSize, 350);

    return () => {
      panel.removeEventListener(
        "transitionend",
        onTransitionEnd as EventListener,
      );
      window.clearTimeout(timeout);
    };
  }, [isExpanded, canvasReady, containerRef, fixCanvasSize]);
}

export const RiveMascot: FC<RiveMascotProps> = ({
  face = "idle",
  size = "100%",
  primaryColor,
  secondaryColor,
  visemeCode = "sil",
  idlePoseRotation = false,
  src = DEFAULT_MASCOT_SRC,
  buffer,
  stateMachine = MASCOT_STATE_MACHINE,
}) => {
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

  useFixCanvasUnderTransform(containerRef, rive);

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

  const basePose = faceToPose(face);

  useEffect(() => {
    setPose(basePose);
  }, [basePose, setPose]);

  const setPoseRef = useRef(setPose);
  setPoseRef.current = setPose;
  useEffect(() => {
    if (!idlePoseRotation || basePose !== "idle") return;
    let timer: number | undefined;
    let current: RivePose = "idle";
    const toIdle = () => {
      current = "idle";
      setPoseRef.current("idle");
      timer = window.setTimeout(
        toAmbient,
        randBetween(AMBIENT_IDLE_MIN_MS, AMBIENT_IDLE_MAX_MS),
      );
    };
    const toAmbient = () => {
      current = pickAmbientPose(current === "idle" ? undefined : current);
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
  }, [idlePoseRotation, basePose]);

  useEffect(() => {
    setMouthVisemeCode(toRiveVisemeCode(visemeCode));
  }, [visemeCode, setMouthVisemeCode]);

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
