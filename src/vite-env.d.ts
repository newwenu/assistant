/// <reference types="vite/client" />
declare const __BUILD_DATE__: string;
declare const __VAULT_ENABLED__: boolean;
declare const __TIMEKEEPER_ENABLED__: boolean;

interface Window {
  __STARTUP_ID__?: string;
}
