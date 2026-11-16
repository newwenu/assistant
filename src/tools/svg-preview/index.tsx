import { useState, useMemo, useCallback, useEffect } from "react";
import { Input, Button, Typography, Space, message } from "antd";
import { CopyOutlined, DownloadOutlined } from "@ant-design/icons";
import {
  DEFAULT_SVG,
  optimizeSvg,
  svgToDataUrl,
  isValidSvg,
} from "./svg-preview.service";
import { useFileDropCapability } from "../framework/capabilities";
import styles from "./svg-preview.module.css";

const { TextArea } = Input;

interface SavingsInfo {
  from: number;
  to: number;
}

export default function SvgPreview() {
  const [svgCode, setSvgCode] = useState(DEFAULT_SVG);
  const [lastSavings, setLastSavings] = useState<SavingsInfo | null>(null);

  const isValid = useMemo(() => {
    if (!svgCode.trim()) return false;
    return isValidSvg(svgCode);
  }, [svgCode]);

  const svgDataUrl = useMemo(() => {
    if (!isValid || !svgCode.trim()) return "";
    return svgToDataUrl(svgCode);
  }, [svgCode, isValid]);

  const optimized = useMemo(() => {
    if (!isValid) return "";
    return optimizeSvg(svgCode);
  }, [svgCode, isValid]);

  const copy = useCallback((text: string) => {
    void navigator.clipboard.writeText(text);
    message.success("已复制");
  }, []);

  const downloadSvg = useCallback(() => {
    if (!svgCode.trim()) return;
    const blob = new Blob([svgCode], { type: "image/svg+xml" });
    const link = document.createElement("a");
    link.download = "image.svg";
    link.href = URL.createObjectURL(blob);
    link.click();
    URL.revokeObjectURL(link.href);
    message.success("已下载 SVG");
  }, [svgCode]);

  const downloadPng = useCallback(() => {
    if (!svgDataUrl) return;
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement("canvas");
      canvas.width = img.naturalWidth || 200;
      canvas.height = img.naturalHeight || 200;
      const ctx = canvas.getContext("2d");
      if (!ctx) return;
      ctx.drawImage(img, 0, 0);
      canvas.toBlob((blob) => {
        if (!blob) return;
        const link = document.createElement("a");
        link.download = "image.png";
        link.href = URL.createObjectURL(blob);
        link.click();
        URL.revokeObjectURL(link.href);
        message.success("已下载 PNG");
      });
    };
    img.src = svgDataUrl;
  }, [svgDataUrl]);

  const useOptimized = useCallback(() => {
    if (optimized && optimized !== svgCode) {
      setLastSavings({ from: svgCode.length, to: optimized.length });
      setSvgCode(optimized);
      message.success("已应用优化");
    }
  }, [optimized, svgCode]);

  const { droppedFiles } = useFileDropCapability();

  useEffect(() => {
    if (droppedFiles && droppedFiles.length > 0) {
      void droppedFiles[0].text().then((text) => {
        setSvgCode(text);
        setLastSavings(null);
        message.success("已加载 SVG 文件");
      });
    }
  }, [droppedFiles]);

  return (
    <div className={styles.root}>
      <div className={styles.editorCol}>
        <div className={styles.editorHeader}>
          <Typography.Text strong>SVG 代码</Typography.Text>
          <Space size="small">
            <Button size="small" onClick={useOptimized} disabled={!isValid}>
              优化
            </Button>
            <Button
              size="small"
              icon={<CopyOutlined />}
              onClick={() => copy(svgCode)}
              disabled={!svgCode}
            >
              复制
            </Button>
          </Space>
        </div>
        <TextArea
          value={svgCode}
          onChange={(e) => {
            setSvgCode(e.target.value);
            setLastSavings(null);
          }}
          rows={20}
          className={styles.textArea}
          style={{ fontFamily: "monospace", fontSize: 12 }}
          status={isValid || !svgCode.trim() ? undefined : "error"}
        />
        {optimized && optimized !== svgCode && (
          <div className={styles.optimizedInfo}>
            <Typography.Text type="secondary">
              优化后: {svgCode.length} → {optimized.length} 字符（节省{" "}
              {Math.round((1 - optimized.length / svgCode.length) * 100)}%）
            </Typography.Text>
          </div>
        )}
        {lastSavings && optimized === svgCode && (
          <div className={styles.optimizedInfo}>
            <Typography.Text type="secondary">
              已优化: {lastSavings.from} → {lastSavings.to} 字符（节省{" "}
              {Math.round((1 - lastSavings.to / lastSavings.from) * 100)}%）
            </Typography.Text>
          </div>
        )}
      </div>

      <div className={styles.previewCol}>
        <Typography.Text strong>预览</Typography.Text>
        <div className={styles.previewBox}>
          {svgDataUrl ? (
            <img
              src={svgDataUrl}
              alt="SVG Preview"
              className={styles.previewImg}
            />
          ) : (
            <div className={styles.placeholder}>
              {svgCode.trim() ? "无效的 SVG" : "输入 SVG 代码"}
            </div>
          )}
        </div>
        <Space>
          <Button
            icon={<DownloadOutlined />}
            onClick={downloadSvg}
            disabled={!isValid}
          >
            下载 SVG
          </Button>
          <Button
            icon={<DownloadOutlined />}
            onClick={downloadPng}
            disabled={!isValid}
          >
            下载 PNG
          </Button>
        </Space>
      </div>
    </div>
  );
}
