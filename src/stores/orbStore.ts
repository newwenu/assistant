import { create } from "zustand";

interface OrbState {
  isExpanded: boolean;
  isHovering: boolean;
  isDragging: boolean;
  isVisible: boolean;
  position: { x: number; y: number } | null;
  expand: () => Promise<void>;
  collapse: () => Promise<void>;
  toggle: () => Promise<void>;
  setHovering: (v: boolean) => void;
  setDragging: (v: boolean) => void;
  setVisible: (v: boolean) => void;
  setPosition: (pos: { x: number; y: number }) => void;
}

export const useOrbStore = create<OrbState>((set, get) => ({
  isExpanded: false,
  isHovering: false,
  isDragging: false,
  isVisible: true,
  position: null,

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
  setVisible: (v: boolean) => set({ isVisible: v }),
  setPosition: (pos: { x: number; y: number }) => set({ position: pos }),
}));
