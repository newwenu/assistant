import React from "react";
import { Routes, Route } from "react-router-dom";
import { Layout } from "antd";
import { Titlebar } from "@/components/Titlebar";
import { FeatureErrorBoundary } from "@/components/FeatureErrorBoundary";
import VaultUnlock from "@/vault/VaultUnlock";

export const VaultUnlockWindow: React.FC = () => {
  return (
    <Layout
      style={{
        height: "100vh",
        background: "var(--color-bg)",
        overflow: "hidden",
      }}
    >
      <Titlebar title="Vault Unlock" />
      <Layout.Content
        style={{
          background: "var(--color-bg)",
          overflow: "auto",
          flex: 1,
          minHeight: 0,
        }}
      >
        <Routes>
          <Route
            path="/vault-unlock"
            element={
              <FeatureErrorBoundary featureName="Vault Unlock">
                <VaultUnlock />
              </FeatureErrorBoundary>
            }
          />
        </Routes>
      </Layout.Content>
    </Layout>
  );
};
