import {
  Fit,
  Layout,
  useRive,
  useViewModel,
  useViewModelInstance,
  useViewModelInstanceColor,
  useViewModelInstanceEnum,
} from "@rive-app/react-webgl2";
import { type FC, useEffect, useMemo, useRef } from "react";

import type { RiveManifestDto } from "@/types/protocol.contract";
import type {
  MascotFace,
  MascotRendererProps,
  MascotScene,
  RiveRendererConfig,
} from "../../core/types";
import { asFace } from "../../core/types";
import { resolveVisemeCode, pickIdleFlourish } from "../../core/resolve";
import { MASCOT_STATE_MACHINE, toRiveVisemeCode } from "./riveMaps";
import {
  EMPTY_RIVE_SCHEMA,
  readRiveSchema,
  resolveRiveBindings,
} from "./riveIntrospect";
import {
  type RiveStateEngine,
  resolveScenePose,
  buildStateEngine,
} from "./riveStateEngine";
import { useRiveManifest } from "./useRiveManifest";
import { useRiveStateMachineNames } from "./useRiveStateMachineNames";

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

interface RiveStageProps {
  face: MascotFace;
  scene?: MascotScene;
  size: number | string;
  primaryColor?: number | null;
  secondaryColor?: number | null;
  visemeCode?: string;
  config: RiveRendererConfig | undefined;
  stateEngine?: RiveStateEngine;
  manifest: RiveManifestDto | null;
  isCustomAsset: boolean;
  stateMachine: string | null;
}

const RiveStage: FC<RiveStageProps> = ({
  face,
  scene,
  size,
  primaryColor,
  secondaryColor,
  visemeCode = "sil",
  config,
  stateEngine,
  manifest,
  isCustomAsset,
  stateMachine,
}) => {
  const idlePoseRotation = config?.idlePoseRotation ?? false;
  const src = config?.src ?? DEFAULT_MASCOT_SRC;
  const buffer = config?.buffer ?? undefined;
  const configFaceToPose = config?.faceToPose;

  const containerRef = useRef<HTMLDivElement>(null);

  const riveSource = buffer ? { buffer } : { src };
  const { rive, RiveComponent } = useRive(
    {
      ...riveSource,
      ...(stateMachine ? { stateMachine } : {}),
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

  const schema = useMemo(
    () =>
      isCustomAsset
        ? readRiveSchema(rive, viewModel, vmInstance)
        : EMPTY_RIVE_SCHEMA,
    [isCustomAsset, rive, viewModel, vmInstance],
  );

  const bindings = useMemo(
    () => resolveRiveBindings(schema, manifest),
    [schema, manifest],
  );

  const { setValue: setPose } = useViewModelInstanceEnum(
    bindings.pose,
    vmInstance,
  );
  const { setValue: setMouthVisemeCode } = useViewModelInstanceEnum(
    bindings.viseme,
    vmInstance,
  );
  const { value: primaryColorValue, setValue: setPrimaryColor } =
    useViewModelInstanceColor(bindings.primaryColor, vmInstance);
  const { value: secondaryColorValue, setValue: setSecondaryColor } =
    useViewModelInstanceColor(bindings.secondaryColor, vmInstance);

  const engine = useMemo<RiveStateEngine>(() => {
    if (stateEngine) return stateEngine;
    if (!isCustomAsset) return buildStateEngine(configFaceToPose);
    const poses = manifest?.poses?.length
      ? manifest.poses
      : (schema.enumValues[bindings.pose] ?? []);
    const visemes = manifest?.visemes?.length
      ? manifest.visemes
      : (schema.enumValues[bindings.viseme] ?? []);
    return buildStateEngine(configFaceToPose, {
      poses,
      visemes,
      idlePoseCycle: manifest?.idlePoseCycle ?? undefined,
      faceToPose: manifest?.faceToPose ?? undefined,
      sceneToPose: manifest?.sceneToPose ?? undefined,
    });
  }, [
    stateEngine,
    isCustomAsset,
    configFaceToPose,
    manifest,
    schema,
    bindings.pose,
    bindings.viseme,
  ]);

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

  const basePose = resolveScenePose(scene, face, engine);

  useEffect(() => {
    setPose(basePose);
  }, [basePose, setPose]);

  const setPoseRef = useRef(setPose);
  setPoseRef.current = setPose;
  const idlePose = engine.states.idle ?? "idle";
  useEffect(() => {
    if (!idlePoseRotation || basePose !== idlePose) return;
    let timer: number | undefined;
    let current: string = idlePose;
    const toIdle = () => {
      current = idlePose;
      setPoseRef.current(idlePose);
      timer = window.setTimeout(
        toAmbient,
        randBetween(AMBIENT_IDLE_MIN_MS, AMBIENT_IDLE_MAX_MS),
      );
    };
    const toAmbient = () => {
      current = pickIdleFlourish(
        engine.idlePoseCycle,
        current === idlePose ? undefined : current,
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
      setPoseRef.current(idlePose);
    };
  }, [idlePoseRotation, basePose, engine, idlePose]);

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

export const RiveMascot: FC<RiveMascotProps> = ({
  face = asFace("idle"),
  scene,
  size = "100%",
  primaryColor,
  secondaryColor,
  visemeCode = "sil",
  rendererConfig,
  stateEngine,
}) => {
  const config = rendererConfig as RiveRendererConfig | undefined;
  const src = config?.src ?? DEFAULT_MASCOT_SRC;
  const isCustomAsset = src !== DEFAULT_MASCOT_SRC;
  const { manifest, ready } = useRiveManifest(
    isCustomAsset ? "rive" : null,
    src,
  );

  const declaredStateMachine =
    config?.stateMachine ?? manifest?.stateMachine ?? null;
  const probeSrc = isCustomAsset && !config?.buffer ? src : null;
  const probe = useRiveStateMachineNames(probeSrc);

  const probed = probe.names;
  const declaredValid =
    declaredStateMachine != null &&
    (probed == null || probed.includes(declaredStateMachine));
  const preferredProbed =
    probed && probed.length > 0
      ? (probed.find(
          (name) => name.toLowerCase() === MASCOT_STATE_MACHINE.toLowerCase(),
        ) ?? probed[0])
      : null;
  const stateMachine =
    (declaredValid ? declaredStateMachine : null) ??
    preferredProbed ??
    (isCustomAsset ? null : MASCOT_STATE_MACHINE);

  const sizeStyle = typeof size === "number" ? `${size}px` : size;

  if (!ready || !probe.ready) {
    return (
      <div
        style={{
          width: sizeStyle,
          height: sizeStyle,
        }}
        data-face={face}
      />
    );
  }

  return (
    <RiveStage
      key={`${src}|${stateMachine ?? "auto"}`}
      face={face}
      scene={scene}
      size={size}
      primaryColor={primaryColor}
      secondaryColor={secondaryColor}
      visemeCode={visemeCode}
      config={config}
      stateEngine={stateEngine}
      manifest={manifest}
      isCustomAsset={isCustomAsset}
      stateMachine={stateMachine}
    />
  );
};
