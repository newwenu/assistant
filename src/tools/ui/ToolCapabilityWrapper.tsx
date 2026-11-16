import { useState, type ReactNode } from "react";
import { InboxOutlined } from "@ant-design/icons";
import { FileDropProvider } from "../framework/capabilities";
import { useFileDrop } from "@/hooks/useFileDrop";
import type { ToolCapabilities } from "../framework/types";
import styles from "./tool-capability.module.css";

interface ToolCapabilityWrapperProps {
  capabilities?: ToolCapabilities;
  children: ReactNode;
}

export function ToolCapabilityWrapper({
  capabilities,
  children,
}: ToolCapabilityWrapperProps) {
  const fileDrop = capabilities?.fileDrop;

  const [droppedFiles, setDroppedFiles] = useState<File[] | null>(null);

  const { isDragging, dropHandlers } = useFileDrop({
    accept: fileDrop?.accept,
    onDrop: setDroppedFiles,
  });

  if (!fileDrop) {
    return <>{children}</>;
  }

  return (
    <FileDropProvider value={{ isDragging, droppedFiles }}>
      <div
        className={`${styles.dropZone} ${isDragging ? styles.dropActive : ""}`}
        {...dropHandlers}
      >
        {isDragging && (
          <div className={styles.dropOverlay}>
            <InboxOutlined className={styles.dropIcon} />
            <span>释放以加载文件</span>
          </div>
        )}
        {children}
      </div>
    </FileDropProvider>
  );
}

export { useFileDropCapability } from "../framework/capabilities";
