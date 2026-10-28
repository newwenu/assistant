import { Form, Input, Button, Typography, Steps, Alert } from "antd";
import {
  SyncOutlined,
  KeyOutlined,
  FolderOpenOutlined,
  CheckCircleOutlined,
  ArrowLeftOutlined,
} from "@ant-design/icons";
import { useTranslation } from "react-i18next";
import { save } from "@tauri-apps/plugin-dialog";
import type { DisasterRecoveryResult as DisasterRecoveryResultType } from "../core/vault.types";
import styles from "../vault.module.css";

interface DisasterRecoveryProps {
  step: 1 | 2 | 3;
  result: DisasterRecoveryResultType | null;
  loading: boolean;
  error: string | null;
  drGitUrl: string;
  onDrGitUrlChange: (value: string) => void;
  drRecoveryKey: string;
  onDrRecoveryKeyChange: (value: string) => void;
  drCommit: string;
  onDrCommitChange: (value: string) => void;
  onNext: () => Promise<void>;
  onCancel: () => void;
  onSetStep: (step: 1 | 2) => void;
}

export default function DisasterRecovery({
  step,
  result,
  loading,
  error,
  drGitUrl,
  onDrGitUrlChange,
  drRecoveryKey,
  onDrRecoveryKeyChange,
  drCommit,
  onDrCommitChange,
  onNext,
  onCancel,
  onSetStep,
}: DisasterRecoveryProps) {
  const { t } = useTranslation();

  const stepLabels = [
    t("vault.disaster_recovery.step_git"),
    t("vault.disaster_recovery.step_key"),
    t("vault.disaster_recovery.step_done"),
  ];

  return (
    <div className={styles.unlockContainer}>
      <SyncOutlined className={styles.lockIcon} spin={loading} />
      <Typography.Title level={4}>
        {t("vault.disaster_recovery.title")}
      </Typography.Title>

      <Steps
        current={step - 1}
        size="small"
        style={{ marginBottom: 24 }}
        items={stepLabels.map((label) => ({ title: label }))}
      />

      {error && (
        <Alert
          type="error"
          title={error}
          showIcon
          style={{ marginBottom: 16 }}
        />
      )}

      {step === 1 && (
        <Form onFinish={() => void onNext()} className={styles.unlockForm}>
          <Typography.Text
            type="secondary"
            style={{ marginBottom: 8, display: "block" }}
          >
            {t("vault.disaster_recovery.git_hint")}
          </Typography.Text>
          <Form.Item>
            <Input
              size="large"
              placeholder={t("vault.disaster_recovery.git_placeholder")}
              value={drGitUrl}
              onChange={(e) => onDrGitUrlChange(e.target.value)}
              autoFocus
            />
          </Form.Item>
          <Form.Item>
            <Input
              size="large"
              placeholder={t("vault.disaster_recovery.commit_placeholder")}
              value={drCommit}
              onChange={(e) => onDrCommitChange(e.target.value)}
            />
          </Form.Item>
          <Form.Item>
            <Button
              type="primary"
              size="large"
              block
              htmlType="submit"
              disabled={loading}
              loading={loading}
            >
              {t("common.next")}
            </Button>
          </Form.Item>
        </Form>
      )}

      {step === 2 && (
        <Form onFinish={() => void onNext()} className={styles.unlockForm}>
          <Form.Item>
            <Input
              size="large"
              placeholder={t("vault.unlock.recovery_key_placeholder")}
              value={drRecoveryKey}
              onChange={(e) => onDrRecoveryKeyChange(e.target.value)}
              prefix={<KeyOutlined />}
              autoFocus
            />
          </Form.Item>
          <Form.Item>
            <Button
              type="primary"
              size="large"
              block
              htmlType="submit"
              icon={<SyncOutlined />}
              disabled={!drRecoveryKey || loading}
              loading={loading}
            >
              {t("vault.disaster_recovery.export")}
            </Button>
          </Form.Item>
        </Form>
      )}

      {step === 3 && result && (
        <div className={styles.unlockForm}>
          {result.integrityWarning && (
            <Alert
              type="warning"
              title={t("vault.disaster_recovery.integrity_warning")}
              showIcon
              style={{ marginBottom: 16 }}
            />
          )}
          <Alert
            type="success"
            title={t("vault.disaster_recovery.success", {
              count: result.entryCount,
            })}
            showIcon
            style={{ marginBottom: 16 }}
          />
          {result.skippedIds.length > 0 && (
            <Alert
              type="warning"
              title={t("vault.disaster_recovery.skipped_warning", {
                count: result.skippedIds.length,
                ids: result.skippedIds.join(", "),
              })}
              showIcon
              style={{ marginBottom: 16 }}
            />
          )}
          <Typography.Text
            type="secondary"
            style={{ display: "block", marginBottom: 8 }}
          >
            {t("vault.disaster_recovery.next_steps")}
          </Typography.Text>
          <Button
            size="large"
            block
            icon={<FolderOpenOutlined />}
            style={{ marginBottom: 8 }}
            onClick={() => {
              void save({
                defaultPath: result.filePath,
              });
            }}
          >
            {t("vault.disaster_recovery.open_file")}
          </Button>
          <Button
            type="primary"
            size="large"
            block
            icon={<CheckCircleOutlined />}
            onClick={onCancel}
          >
            {t("common.done")}
          </Button>
        </div>
      )}

      <Button
        type="link"
        icon={<ArrowLeftOutlined />}
        onClick={() => {
          if (step === 1) {
            onCancel();
          } else {
            onSetStep((step - 1) as 1 | 2);
          }
        }}
        style={{ marginTop: 16 }}
      >
        {t("common.back")}
      </Button>
    </div>
  );
}
