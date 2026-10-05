import { create } from "zustand";

export type ExpandDirection = "left" | "right";
export type ExpandVertical = "up" | "down";

interface OrbState {
  isExpanded: boolean;
  isHovering: boolean;
  isDragging: boolean;
  isBallVisible: boolean;
  isVisible: boolean;
  clickThrough: boolean | null;
  position: { x: number; y: number } | null;
  expandDirection: ExpandDirection;
  expandVertical: ExpandVertical;
  expand: () => Promise<void>;
  collapse: () => Promise<void>;
  toggle: () => Promise<void>;
  setHovering: (v: boolean) => void;
  setDragging: (v: boolean) => void;
  setClickThrough: (v: boolean) => void;
  setBallVisible: (v: boolean) => void;
  setVisible: (v: boolean) => void;
  setPosition: (pos: { x: number; y: number }) => void;
  setExpandDirection: (dir: ExpandDirection) => void;
  setExpandVertical: (v: ExpandVertical) => void;
}

export const useOrbStore = create<OrbState>((set, get) => ({
  isExpanded: false,
  isHovering: false,
  isDragging: false,
  isBallVisible: true,
  isVisible: true,
  clickThrough: null,
  position: null,
  expandDirection: "right",
  expandVertical: "down",

  expand: async () => {
    set({ isExpanded: true });
  },

  collapse: async () => {
    set({ isExpanded: false, isHovering: false });
  },

  toggle: async () => {
    set({ isVisible: !get().isVisible });
  },

  setHovering: (v: boolean) => set({ isHovering: v }),
  setDragging: (v: boolean) => set({ isDragging: v }),
  setClickThrough: (v: boolean) => set({ clickThrough: v }),
  setBallVisible: (v: boolean) => set({ isBallVisible: v }),
  setVisible: (v: boolean) => set({ isVisible: v }),
  setPosition: (pos: { x: number; y: number }) => set({ position: pos }),
  setExpandDirection: (dir: ExpandDirection) => set({ expandDirection: dir }),
  setExpandVertical: (v: ExpandVertical) => set({ expandVertical: v }),
}));
