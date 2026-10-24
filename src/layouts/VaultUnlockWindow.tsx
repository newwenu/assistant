import React from "react";
import { Routes, Route } from "react-router-dom";
import { Layout, Spin } from "antd";
import { Titlebar } from "@/components/Titlebar";
import { FeatureErrorBoundary } from "@/components/FeatureErrorBoundary";
import { useVaultStatusReady } from "@/vault/core/useVaultStatusReady";
import VaultUnlock from "@/vault/VaultUnlock";

export const VaultUnlockWindow: React.FC = () => {
  const statusReady = useVaultStatusReady();

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
        {statusReady ? (
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
        ) : (
          <div
            style={{
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
              height: "100%",
            }}
          >
            <Spin size="large" />
          </div>
        )}
      </Layout.Content>
    </Layout>
  );
};
