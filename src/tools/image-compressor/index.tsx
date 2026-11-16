import { useState, useCallback, useRef, useEffect } from "react";
import {
  Slider,
  InputNumber,
  Button,
  Typography,
  Upload,
  Space,
  Select,
  message,
} from "antd";
import { UploadOutlined, DownloadOutlined } from "@ant-design/icons";
import {
  compressImage,
  formatSize,
  FORMAT_OPTIONS,
  type CompressedResult,
} from "./image-compressor.service";
import { useFileDropCapability } from "../framework/capabilities";
import styles from "./image-compressor.module.css";

export default function ImageCompressor() {
  const [file, setFile] = useState<File | null>(null);
  const [originalUrl, setOriginalUrl] = useState("");
  const [originalSize, setOriginalSize] = useState(0);
  const [quality, setQuality] = useState(80);
  const [maxWidth, setMaxWidth] = useState(0);
  const [maxHeight, setMaxHeight] = useState(0);
  const [format, setFormat] = useState("image/jpeg");
  const [result, setResult] = useState<CompressedResult | null>(null);
  const [compressing, setCompressing] = useState(false);
  const prevUrlRef = useRef("");

  useEffect(() => {
    return () => {
      if (prevUrlRef.current) URL.revokeObjectURL(prevUrlRef.current);
    };
  }, []);

  const handleFile = useCallback((f: File) => {
    if (!f.type.startsWith("image/")) {
      message.error("请选择图片文件");
      return;
    }
    if (prevUrlRef.current) URL.revokeObjectURL(prevUrlRef.current);
    const url = URL.createObjectURL(f);
    prevUrlRef.current = url;
    setFile(f);
    setOriginalUrl(url);
    setOriginalSize(f.size);
    setResult(null);
  }, []);

  const handleCompress = useCallback(async () => {
    if (!file) return;
    setCompressing(true);
    try {
      const r = await compressImage(
        file,
        quality / 100,
        maxWidth,
        maxHeight,
        format,
      );
      setResult(r);
    } catch (e) {
      message.error(e instanceof Error ? e.message : "压缩失败");
    } finally {
      setCompressing(false);
    }
  }, [file, quality, maxWidth, maxHeight, format]);

  const download = useCallback(() => {
    if (!result) return;
    const ext =
      format === "image/png" ? "png" : format === "image/webp" ? "webp" : "jpg";
    const link = document.createElement("a");
    link.download = `compressed_${file?.name?.replace(/\.[^.]+$/, "") ?? "image"}.${ext}`;
    link.href = result.dataUrl;
    link.click();
    message.success("已下载");
  }, [result, file, format]);

  const savings =
    result && originalSize > 0
      ? Math.round((1 - result.size / originalSize) * 100)
      : 0;

  const { droppedFiles } = useFileDropCapability();

  useEffect(() => {
    if (droppedFiles && droppedFiles.length > 0) {
      handleFile(droppedFiles[0]);
    }
  }, [droppedFiles, handleFile]);

  return (
    <div className={styles.root}>
      <div className={styles.configCol}>
        <Upload
          beforeUpload={(f) => {
            handleFile(f);
            return false;
          }}
          showUploadList={false}
          accept="image/*"
          maxCount={1}
        >
          <Button icon={<UploadOutlined />}>选择图片</Button>
        </Upload>

        {file && (
          <Typography.Text type="secondary" className={styles.fileName}>
            {file.name}（{formatSize(originalSize)}）
          </Typography.Text>
        )}

        <div className={styles.optionRow}>
          <Typography.Text strong className={styles.optionLabel}>
            输出格式
          </Typography.Text>
          <Select
            value={format}
            onChange={setFormat}
            options={FORMAT_OPTIONS}
            disabled={!file}
            style={{ flex: 1 }}
          />
        </div>

        <div className={styles.optionRow}>
          <Typography.Text strong className={styles.optionLabel}>
            质量
          </Typography.Text>
          <Slider
            min={1}
            max={100}
            value={quality}
            onChange={setQuality}
            disabled={!file || format === "image/png"}
            style={{ flex: 1 }}
          />
          <InputNumber
            min={1}
            max={100}
            value={quality}
            onChange={(v) => v && setQuality(v)}
            disabled={!file || format === "image/png"}
            style={{ width: 60 }}
          />
        </div>

        <div className={styles.optionRow}>
          <Typography.Text strong className={styles.optionLabel}>
            最大宽度
          </Typography.Text>
          <InputNumber
            min={0}
            value={maxWidth}
            onChange={(v) => setMaxWidth(v ?? 0)}
            disabled={!file}
            placeholder="0 = 不限"
            style={{ flex: 1 }}
          />
        </div>

        <div className={styles.optionRow}>
          <Typography.Text strong className={styles.optionLabel}>
            最大高度
          </Typography.Text>
          <InputNumber
            min={0}
            value={maxHeight}
            onChange={(v) => setMaxHeight(v ?? 0)}
            disabled={!file}
            placeholder="0 = 不限"
            style={{ flex: 1 }}
          />
        </div>

        <Button
          type="primary"
          onClick={() => {
            void handleCompress();
          }}
          loading={compressing}
          disabled={!file}
          style={{ width: "100%" }}
        >
          压缩
        </Button>
      </div>

      <div className={styles.previewCol}>
        {originalUrl ? (
          <div className={styles.imageCard}>
            <Typography.Text strong>原图</Typography.Text>
            <img
              src={originalUrl}
              alt="original"
              className={styles.previewImg}
            />
          </div>
        ) : (
          <div className={styles.placeholder}>选择图片开始压缩</div>
        )}

        {result && (
          <div className={styles.imageCard}>
            <div className={styles.resultHeader}>
              <Typography.Text strong>压缩后</Typography.Text>
              <Space>
                <Typography.Text type="secondary">
                  {formatSize(result.size)} (
                  {savings > 0 ? `-${savings}%` : `+${Math.abs(savings)}%`})
                </Typography.Text>
                <Typography.Text type="secondary">
                  {result.width}×{result.height}
                </Typography.Text>
              </Space>
            </div>
            <img
              src={result.dataUrl}
              alt="compressed"
              className={styles.previewImg}
            />
            <Button
              icon={<DownloadOutlined />}
              onClick={download}
              style={{ marginTop: 8 }}
            >
              下载
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
