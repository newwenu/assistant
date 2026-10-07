import { useState, useEffect, useCallback } from "react";
import { Input, Select, Button, Typography, ColorPicker, message } from "antd";
import { DownloadOutlined } from "@ant-design/icons";
import QRCode from "qrcode";
import type { QRCodeErrorCorrectionLevel } from "qrcode";
import styles from "./qrcode-generator.module.css";

const { TextArea } = Input;

const ERROR_LEVELS: { value: QRCodeErrorCorrectionLevel; label: string }[] = [
  { value: "L", label: "低 (L ~7%)" },
  { value: "M", label: "中 (M ~15%)" },
  { value: "Q", label: "较高 (Q ~25%)" },
  { value: "H", label: "高 (H ~30%)" },
];

export default function QRCodeGenerator() {
  const [text, setText] = useState("https://example.com");
  const [foreground, setForeground] = useState("#000000");
  const [background, setBackground] = useState("#ffffff");
  const [errorLevel, setErrorLevel] = useState<QRCodeErrorCorrectionLevel>("M");
  const [dataUrl, setDataUrl] = useState("");

  useEffect(() => {
    if (!text.trim()) {
      setDataUrl("");
      return;
    }
    QRCode.toDataURL(text.trim(), {
      width: 512,
      margin: 2,
      color: {
        dark: foreground,
        light: background,
      },
      errorCorrectionLevel: errorLevel,
    })
      .then(setDataUrl)
      .catch(() => setDataUrl(""));
  }, [text, foreground, background, errorLevel]);

  const download = useCallback(() => {
    if (!dataUrl) return;
    const link = document.createElement("a");
    link.download = "qrcode.png";
    link.href = dataUrl;
    link.click();
    message.success("已下载");
  }, [dataUrl]);

  const copyImage = useCallback(async () => {
    if (!dataUrl) return;
    try {
      const res = await fetch(dataUrl);
      const blob = await res.blob();
      await navigator.clipboard.write([
        new ClipboardItem({ "image/png": blob }),
      ]);
      message.success("已复制图片");
    } catch {
      message.error("复制失败，请尝试下载");
    }
  }, [dataUrl]);

  return (
    <div className={styles.root}>
      <div className={styles.configCol}>
        <TextArea
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="输入文本或 URL..."
          autoSize={{ minRows: 2, maxRows: 6 }}
          className={styles.textInput}
        />

        <div className={styles.optionRow}>
          <Typography.Text strong className={styles.optionLabel}>
            前景色
          </Typography.Text>
          <ColorPicker
            value={foreground}
            onChange={(_, hex) => setForeground(hex)}
            size="small"
          />
        </div>

        <div className={styles.optionRow}>
          <Typography.Text strong className={styles.optionLabel}>
            背景色
          </Typography.Text>
          <ColorPicker
            value={background}
            onChange={(_, hex) => setBackground(hex)}
            size="small"
          />
        </div>

        <div className={styles.optionRow}>
          <Typography.Text strong className={styles.optionLabel}>
            容错等级
          </Typography.Text>
          <Select
            value={errorLevel}
            onChange={setErrorLevel}
            options={ERROR_LEVELS}
            style={{ width: 160 }}
          />
        </div>
      </div>

      <div className={styles.previewCol}>
        {dataUrl ? (
          <img src={dataUrl} alt="QR Code" className={styles.qrImage} />
        ) : (
          <div className={styles.placeholder}>输入文本生成二维码</div>
        )}
        <div className={styles.actionRow}>
          <Button
            icon={<DownloadOutlined />}
            onClick={download}
            disabled={!dataUrl}
          >
            下载 PNG
          </Button>
          <Button onClick={copyImage} disabled={!dataUrl}>
            复制图片
          </Button>
        </div>
      </div>
    </div>
  );
}
