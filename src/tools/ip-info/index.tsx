import { useState, useMemo } from "react";
import {
  Input,
  Descriptions,
  Button,
  Typography,
  App,
  Spin,
  Alert,
  InputNumber,
} from "antd";
import { CopyOutlined, SearchOutlined } from "@ant-design/icons";
import {
  isValidIpv4,
  isValidIpv6,
  getIpv4Conversions,
  getIpv6Conversions,
  getSubnetInfo,
} from "./ip-info.service";
import type { SubnetInfo } from "./ip-info.service";

import styles from "./ip-info.module.css";

interface IpGeoInfo {
  ip: string;
  city?: string;
  region?: string;
  country?: string;
  loc?: string;
  org?: string;
  timezone?: string;
  postal?: string;
}

export default function IpInfo() {
  const { message } = App.useApp();
  const [ipInput, setIpInput] = useState("192.168.1.1");
  const [cidr, setCidr] = useState(24);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");
  const [geoInfo, setGeoInfo] = useState<IpGeoInfo | null>(null);

  const trimmed = ipInput.trim();
  const isIpv4 = trimmed !== "" && isValidIpv4(trimmed);
  const isIpv6 = trimmed !== "" && isValidIpv6(trimmed);
  const valid = trimmed === "" || isIpv4 || isIpv6;

  const ipv4Conversions = useMemo(() => {
    if (!isIpv4) return null;
    return getIpv4Conversions(trimmed);
  }, [trimmed, isIpv4]);

  const ipv6Conversions = useMemo(() => {
    if (!isIpv6) return null;
    return getIpv6Conversions(trimmed);
  }, [trimmed, isIpv6]);

  const subnetInfo = useMemo(() => {
    if (!isIpv4) return null;
    return getSubnetInfo(trimmed, cidr);
  }, [trimmed, isIpv4, cidr]);

  const lookup = async () => {
    if (!trimmed) return;
    setLoading(true);
    setError("");
    setGeoInfo(null);
    try {
      const resp = await fetch(`https://ipinfo.io/${trimmed}/json`);
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
      const data = (await resp.json()) as IpGeoInfo & { bogon?: boolean };
      if (data.bogon) {
        setError("该 IP 为保留/私有地址，无地理位置信息");
      } else {
        setGeoInfo(data);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "查询失败");
    } finally {
      setLoading(false);
    }
  };

  const copy = (text: string) => {
    void navigator.clipboard.writeText(text);
    message.success("已复制");
  };

  const renderValue = (val: string) => (
    <div className={styles.valueRow}>
      <code>{val}</code>
      <Button
        type="text"
        size="small"
        icon={<CopyOutlined />}
        onClick={() => copy(val)}
      />
    </div>
  );

  const renderSubnet = (info: SubnetInfo) => (
    <Descriptions
      size="small"
      column={1}
      bordered
      className={styles.desc}
      items={[
        {
          key: "net",
          label: "网络地址",
          children: renderValue(info.networkAddress),
        },
        {
          key: "bc",
          label: "广播地址",
          children: renderValue(info.broadcastAddress),
        },
        {
          key: "mask",
          label: "子网掩码",
          children: renderValue(info.subnetMask),
        },
        {
          key: "maskBin",
          label: "掩码(二进制)",
          children: renderValue(info.subnetMaskBinary),
        },
        { key: "cidr", label: "CIDR", children: renderValue(`/${info.cidr}`) },
        {
          key: "wild",
          label: "通配符掩码",
          children: renderValue(info.wildcardMask),
        },
        {
          key: "first",
          label: "首个主机",
          children: renderValue(info.firstHost),
        },
        {
          key: "last",
          label: "末个主机",
          children: renderValue(info.lastHost),
        },
        {
          key: "total",
          label: "总地址数",
          children: info.totalHosts.toLocaleString(),
        },
        {
          key: "usable",
          label: "可用主机数",
          children: info.usableHosts.toLocaleString(),
        },
        { key: "class", label: "IP 类别", children: info.ipClass },
      ]}
    />
  );

  return (
    <div>
      <div className={styles.inputRow}>
        <Input
          value={ipInput}
          onChange={(e) => setIpInput(e.target.value)}
          placeholder="输入 IPv4 或 IPv6 地址..."
          status={valid ? undefined : "error"}
          style={{ fontFamily: "monospace" }}
          onKeyDown={(e) => {
            if (e.key === "Enter") void lookup();
          }}
        />
        <Button
          type="primary"
          icon={<SearchOutlined />}
          onClick={() => {
            void lookup();
          }}
          loading={loading}
          disabled={!valid || !trimmed}
        >
          查询
        </Button>
      </div>

      {!valid && (
        <Alert type="error" title="无效的 IP 地址" className={styles.alert} />
      )}

      {ipv4Conversions && (
        <div className={styles.section}>
          <Typography.Text strong>地址转换</Typography.Text>
          <Descriptions
            size="small"
            column={1}
            bordered
            className={styles.desc}
            items={[
              {
                key: "dec",
                label: "十进制",
                children: renderValue(ipv4Conversions.decimal),
              },
              {
                key: "hex",
                label: "十六进制",
                children: renderValue(ipv4Conversions.hex),
              },
              {
                key: "bin",
                label: "二进制",
                children: renderValue(ipv4Conversions.binary),
              },
              {
                key: "v6m",
                label: "IPv6 映射",
                children: renderValue(ipv4Conversions.ipv6Mapped),
              },
              {
                key: "v6f",
                label: "IPv6 完整",
                children: renderValue(ipv4Conversions.ipv6Full),
              },
            ]}
          />
        </div>
      )}

      {ipv6Conversions && (
        <div className={styles.section}>
          <Typography.Text strong>地址转换</Typography.Text>
          <Descriptions
            size="small"
            column={1}
            bordered
            className={styles.desc}
            items={[
              {
                key: "exp",
                label: "完整展开",
                children: renderValue(ipv6Conversions.expanded),
              },
              {
                key: "comp",
                label: "压缩格式",
                children: renderValue(ipv6Conversions.compressed),
              },
              {
                key: "dec",
                label: "十进制",
                children: renderValue(ipv6Conversions.decimal),
              },
            ]}
          />
        </div>
      )}

      {isIpv4 && (
        <div className={styles.section}>
          <div className={styles.subnetHeader}>
            <Typography.Text strong>子网计算</Typography.Text>
            <div className={styles.cidrRow}>
              <Typography.Text type="secondary">CIDR</Typography.Text>
              <InputNumber
                min={0}
                max={32}
                value={cidr}
                onChange={(v) => v != null && setCidr(v)}
                className={styles.cidrInput}
              />
            </div>
          </div>
          {subnetInfo && renderSubnet(subnetInfo)}
        </div>
      )}

      {loading && <Spin className={styles.spin} />}

      {error && <Alert type="warning" title={error} className={styles.alert} />}

      {geoInfo && (
        <div className={styles.section}>
          <Typography.Text strong>地理位置</Typography.Text>
          <Descriptions
            size="small"
            column={1}
            bordered
            className={styles.desc}
            items={[
              { key: "ip", label: "IP", children: geoInfo.ip },
              ...(geoInfo.city
                ? [{ key: "city", label: "城市", children: geoInfo.city }]
                : []),
              ...(geoInfo.region
                ? [{ key: "region", label: "地区", children: geoInfo.region }]
                : []),
              ...(geoInfo.country
                ? [{ key: "country", label: "国家", children: geoInfo.country }]
                : []),
              ...(geoInfo.loc
                ? [{ key: "loc", label: "坐标", children: geoInfo.loc }]
                : []),
              ...(geoInfo.org
                ? [{ key: "org", label: "组织/ISP", children: geoInfo.org }]
                : []),
              ...(geoInfo.timezone
                ? [{ key: "tz", label: "时区", children: geoInfo.timezone }]
                : []),
              ...(geoInfo.postal
                ? [{ key: "postal", label: "邮编", children: geoInfo.postal }]
                : []),
            ]}
          />
        </div>
      )}
    </div>
  );
}
