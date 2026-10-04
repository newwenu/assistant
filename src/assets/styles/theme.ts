import type { ThemeConfig } from "antd";
import { theme } from "antd";

const commonToken = {
  borderRadius: 6,
  fontSize: 14,
  colorPrimary: "#1677ff",
};

export const lightTheme: ThemeConfig = {
  algorithm: theme.defaultAlgorithm,
  token: {
    ...commonToken,
    colorBgLayout: "#f5f5f5",
    colorBgContainer: "#ffffff",
    colorBgElevated: "#ffffff",
  },
  components: {
    Menu: { itemHeight: 36, itemMarginBlock: 2 },
    Button: { controlHeight: 32 },
    Input: { controlHeight: 32 },
  },
};

export const darkTheme: ThemeConfig = {
  algorithm: theme.darkAlgorithm,
  token: {
    ...commonToken,
    colorPrimary: "#1668dc",
    colorBgLayout: "#0a0a0a",
    colorBgContainer: "#141414",
    colorBgElevated: "#1f1f1f",
  },
  components: {
    Menu: { itemHeight: 36, itemMarginBlock: 2 },
    Button: { controlHeight: 32 },
    Input: { controlHeight: 32 },
  },
};
