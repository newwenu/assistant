import { create } from "zustand";

export type ExpandDirection = "left" | "right";
export type ExpandVertical = "up" | "down";

interface OrbState {
  isExpanded: boolean;
  isHovering: boolean;
  isDragging: boolean;
  isBallVisible: boolean;
  isPinned: boolean;
  position: { x: number; y: number } | null;
  expandDirection: ExpandDirection;
  expandVertical: ExpandVertical;
  expand: () => void;
  collapse: () => void;
  setHovering: (v: boolean) => void;
  setDragging: (v: boolean) => void;
  setBallVisible: (v: boolean) => void;
  setPinned: (v: boolean) => void;
  setPosition: (pos: { x: number; y: number }) => void;
  setExpandDirection: (dir: ExpandDirection) => void;
  setExpandVertical: (v: ExpandVertical) => void;
}

export const useOrbStore = create<OrbState>((set) => ({
  isExpanded: false,
  isHovering: false,
  isDragging: false,
  isBallVisible: true,
  isPinned: false,
  position: null,
  expandDirection: "right",
  expandVertical: "down",

  expand: () => {
    set({ isExpanded: true });
  },

  collapse: () => {
    set({ isExpanded: false, isHovering: false });
  },

  setHovering: (v: boolean) => set({ isHovering: v }),
  setDragging: (v: boolean) => set({ isDragging: v }),
  setBallVisible: (v: boolean) => set({ isBallVisible: v }),
  setPinned: (v: boolean) => set({ isPinned: v }),
  setPosition: (pos: { x: number; y: number }) => set({ position: pos }),
  setExpandDirection: (dir: ExpandDirection) => set({ expandDirection: dir }),
  setExpandVertical: (v: ExpandVertical) => set({ expandVertical: v }),
}));
