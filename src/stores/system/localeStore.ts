import { create } from "zustand";

interface LocaleState {
  language: string;
  setLocale: (language: string) => void;
}

export const useLocaleStore = create<LocaleState>((set) => ({
  language: "zh-CN",
  setLocale: (language: string) => set({ language }),
}));
