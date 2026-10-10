import type React from "react";

interface SettingsPageShellProps {
  children: React.ReactNode;
}

export const SettingsPageShell: React.FC<SettingsPageShellProps> = ({
  children,
}) => (
  <div style={{ maxWidth: 700, margin: "0 auto", padding: 24 }}>{children}</div>
);
