import { useState, useEffect } from "react";
import { Input, Button, Select, Typography, Alert, Space, App } from "antd";
import { LinkOutlined, SearchOutlined } from "@ant-design/icons";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import type { DragDropEvent } from "@tauri-apps/api/webview";
import type { Event } from "@tauri-apps/api/event";
import { os } from "../framework/os";
import {
  LINK_KIND_OPTIONS,
  KIND_DESCRIPTIONS,
  type LinkKind,
  type CheckResult,
  type CreateResult,
} from "./fs-link.service";
import styles from "./fs-link.module.css";

const { TextArea } = Input;

/** 绝对路径判定：盘符 / POSIX 根 / UNC。相对路径不按工作区解析（I38h §10.4）。 */
const isAbsolutePath = (p: string) =>
  /^[A-Za-z]:[\\/]/.test(p) || p.startsWith("/") || p.startsWith("\\\\");

export default function FsLink() {
  const { message } = App.useApp();
  const [source, setSource] = useState("");
  const [target, setTarget] = useState("");
  const [kind, setKind] = useState<LinkKind>("symlink");
  const [checkResult, setCheckResult] = useState<CheckResult | null>(null);
  const [createResult, setCreateResult] = useState<CreateResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [dragOver, setDragOver] = useState(false);

  useEffect(() => {
    const webview = getCurrentWebview();
    let disposed = false;

    const setup = async () => {
      const unlisten = await webview.onDragDropEvent(
        (event: Event<DragDropEvent>) => {
          if (disposed) return;
          const payload = event.payload;
          if (payload.type === "drop") {
            const path = payload.paths[0] ?? "";
            if (path) setTarget(path);
            setDragOver(false);
          } else if (payload.type === "enter") {
            setDragOver(true);
          } else if (payload.type === "leave") {
            setDragOver(false);
          }
        },
      );

      return unlisten;
    };

    let unlistenFn: (() => void) | undefined;
    void setup().then((fn) => {
      unlistenFn = fn;
    });

    return () => {
      disposed = true;
      unlistenFn?.();
    };
  }, []);

  const handleCreate = async () => {
    if (!source || !target) {
      message.warning("请填写源路径和目标路径");
      return;
    }
    if (!isAbsolutePath(source) || !isAbsolutePath(target)) {
      message.warning("请填写绝对路径（本工具不按工作区解析相对路径）");
      return;
    }
    setLoading(true);
    setError(null);
    setCreateResult(null);
    setCheckResult(null);
    try {
      const result = await os.fsLink.create(source, target, kind);
      if (result.ok && result.data) {
        setCreateResult(result.data);
        message.success("链接创建成功");
      } else {
        setError(result.error ?? "创建失败");
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handleCheck = async () => {
    if (!source) {
      message.warning("请填写要检查的路径");
      return;
    }
    if (!isAbsolutePath(source)) {
      message.warning("请填写绝对路径（本工具不按工作区解析相对路径）");
      return;
    }
    setLoading(true);
    setError(null);
    setCreateResult(null);
    setCheckResult(null);
    try {
      const result = await os.fsLink.check(source);
      if (result.ok && result.data) {
        setCheckResult(result.data);
      } else {
        setError(result.error ?? "检查失败");
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className={`${styles.container} ${dragOver ? styles.dropActive : ""}`}>
      {dragOver && (
        <div className={styles.dropOverlay}>
          <Typography.Text strong className={styles.dropText}>
            释放以填入目标路径
          </Typography.Text>
        </div>
      )}
      <div className={styles.section}>
        <Typography.Text strong>源路径 — 链接将被创建在此位置</Typography.Text>
        <Typography.Text type="secondary" className={styles.fieldHint}>
          即新链接本身的路径。创建后，访问此路径等同于访问目标路径。此路径当前不应已存在。必须填写绝对路径，相对路径不会按工作区解析。
        </Typography.Text>
        <TextArea
          value={source}
          onChange={(e) => setSource(e.target.value)}
          placeholder="例如: G:\project\link_name  或  /home/user/link_name"
          rows={2}
          className={styles.input}
        />
      </div>

      <div className={styles.section}>
        <Typography.Text strong>
          目标路径 — 链接指向的实际文件或目录
        </Typography.Text>
        <Typography.Text type="secondary" className={styles.fieldHint}>
          即链接引用的真实位置。此路径必须已存在，否则创建会失败。必须填写绝对路径。可拖入文件或文件夹到页面任意位置。
        </Typography.Text>
        <TextArea
          value={target}
          onChange={(e) => setTarget(e.target.value)}
          placeholder="拖入文件/文件夹，或手动输入: G:\data\real_folder"
          rows={2}
          className={styles.input}
        />
      </div>

      <div className={styles.section}>
        <Typography.Text strong>链接类型</Typography.Text>
        <Select
          value={kind}
          onChange={(v) => setKind(v)}
          options={LINK_KIND_OPTIONS}
          className={styles.select}
        />
        <Typography.Text type="secondary" className={styles.fieldHint}>
          {KIND_DESCRIPTIONS[kind]}
        </Typography.Text>
      </div>

      <Space className={styles.actions}>
        <Button
          type="primary"
          icon={<LinkOutlined />}
          loading={loading}
          onClick={() => void handleCreate()}
        >
          创建链接
        </Button>
        <Button
          icon={<SearchOutlined />}
          loading={loading}
          onClick={() => void handleCheck()}
        >
          检查链接
        </Button>
      </Space>

      {error && (
        <Alert
          type="error"
          title="操作失败"
          description={error}
          showIcon
          className={styles.alert}
        />
      )}

      {createResult && (
        <Alert
          type="success"
          title="链接创建成功"
          description={
            <div>
              <div>源路径（链接）: {createResult.source}</div>
              <div>目标路径（实际）: {createResult.target}</div>
              <div>链接类型: {createResult.kind}</div>
            </div>
          }
          showIcon
          className={styles.alert}
        />
      )}

      {checkResult && (
        <Alert
          type={checkResult.isLink ? "success" : "info"}
          title={checkResult.isLink ? "检测到链接" : "未检测到链接"}
          description={
            <div>
              <div>检查路径: {checkResult.source}</div>
              <div>是否为链接: {checkResult.isLink ? "是" : "否"}</div>
              {checkResult.kind && <div>链接类型: {checkResult.kind}</div>}
              {checkResult.target && <div>指向目标: {checkResult.target}</div>}
            </div>
          }
          showIcon
          className={styles.alert}
        />
      )}

      <div className={styles.hint}>
        <Typography.Text type="secondary">
          提示：Windows 创建符号链接 (Symlink) 需要开启开发者模式或拥有
          SeCreateSymbolicLinkPrivilege 特权。硬链接和目录联接 (Junction)
          不需要额外特权。硬链接仅支持文件，联接仅支持目录。
        </Typography.Text>
      </div>
    </div>
  );
}
