import { useState, useMemo, useCallback, useRef, useEffect } from "react";
import { Input, Button, Typography, Space, message } from "antd";
import { CopyOutlined, DownloadOutlined } from "@ant-design/icons";
import styles from "./svg-preview.module.css";

const { TextArea } = Input;

const DEFAULT_SVG = `<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200">
  <circle cx="100" cy="100" r="80" fill="#4f46e5" opacity="0.8"/>
  <circle cx="100" cy="100" r="50" fill="#818cf8" opacity="0.8"/>
  <circle cx="100" cy="100" r="20" fill="#c7d2fe"/>
</svg>`;

function optimizeSvg(svg: string): string {
  return svg
    .replace(/<!--[\s\S]*?-->/g, "")
    .replace(/\s+/g, " ")
    .replace(/>\s+</g, "><")
    .replace(/\s+\/>/g, "/>")
    .trim();
}

export default function SvgPreview() {
  const [svgCode, setSvgCode] = useState(DEFAULT_SVG);
  const objectUrlRef = useRef("");

  const isValid = useMemo(() => {
    try {
      const parser = new DOMParser();
      const doc = parser.parseFromString(svgCode, "image/svg+xml");
      return !doc.querySelector("parsererror");
    } catch {
      return false;
    }
  }, [svgCode]);

  const svgDataUrl = useMemo(() => {
    if (objectUrlRef.current) {
      URL.revokeObjectURL(objectUrlRef.current);
      objectUrlRef.current = "";
    }
    if (!isValid || !svgCode.trim()) return "";
    const blob = new Blob([svgCode], { type: "image/svg+xml" });
    const url = URL.createObjectURL(blob);
    objectUrlRef.current = url;
    return url;
  }, [svgCode, isValid]);

  useEffect(() => {
    return () => {
      if (objectUrlRef.current) URL.revokeObjectURL(objectUrlRef.current);
    };
  }, []);

  const optimized = useMemo(() => {
    if (!isValid) return "";
    return optimizeSvg(svgCode);
  }, [svgCode, isValid]);

  const copy = useCallback((text: string) => {
    navigator.clipboard.writeText(text);
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
    if (optimized) {
      setSvgCode(optimized);
      message.success("已应用优化");
    }
  }, [optimized]);

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
          onChange={(e) => setSvgCode(e.target.value)}
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