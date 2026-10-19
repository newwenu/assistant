export function isValidIpv4(ip: string): boolean {
  return /^((25[0-5]|(2[0-4]|1\d|[1-9]|)\d)\.?\b){4}$/.test(ip.trim());
}

export function isValidIpv6(ip: string): boolean {
  const trimmed = ip.trim();
  if (!trimmed || !trimmed.includes(":")) return false;
  const expanded = expandIpv6(trimmed);
  if (!expanded) return false;
  const groups = expanded.split(":");
  return groups.length === 8 && groups.every((g) => /^[0-9a-fA-F]{4}$/.test(g));
}

function expandIpv6(ip: string): string | null {
  let addr = ip.trim();
  if (addr.startsWith("[") && addr.endsWith("]")) {
    addr = addr.slice(1, -1);
  }
  const doubleColonIndex = addr.indexOf("::");
  if (doubleColonIndex !== -1) {
    if (addr.indexOf("::", doubleColonIndex + 2) !== -1) return null;
    const before = addr.slice(0, doubleColonIndex);
    const after = addr.slice(doubleColonIndex + 2);
    const beforeGroups = before ? before.split(":") : [];
    const afterGroups = after ? after.split(":") : [];
    const missing = 8 - beforeGroups.length - afterGroups.length;
    if (missing < 1) return null;
    const full = [
      ...beforeGroups,
      ...Array<string>(missing).fill("0"),
      ...afterGroups,
    ].join(":");
    return normalizeIpv6Groups(full);
  }
  return normalizeIpv6Groups(addr);
}

function normalizeIpv6Groups(addr: string): string | null {
  const groups = addr.split(":");
  if (groups.length !== 8) return null;
  return groups
    .map((g) => {
      if (g.length > 4) return null;
      return g.padStart(4, "0");
    })
    .join(":");
}

export function ipv4ToInt(ip: string): bigint {
  const parts = ip.split(".").map(Number);
  return (
    BigInt(parts[0]) * 16777216n +
    BigInt(parts[1]) * 65536n +
    BigInt(parts[2]) * 256n +
    BigInt(parts[3])
  );
}

export function intToIpv4(n: bigint): string {
  return [
    Number((n >> 24n) & 255n),
    Number((n >> 16n) & 255n),
    Number((n >> 8n) & 255n),
    Number(n & 255n),
  ].join(".");
}

export function ipv4ToBinary(ip: string): string {
  return ip
    .split(".")
    .map((p) => Number(p).toString(2).padStart(8, "0"))
    .join(".");
}

export function ipv4ToHex(ip: string): string {
  const parts = ip.split(".").map(Number);
  const hex = parts.map((p) => p.toString(16).padStart(2, "0"));
  return `0000:0000:0000:0000:0000:${hex[0]}${hex[1]}:${hex[2]}${hex[3]}`;
}

export interface Ipv4Conversions {
  decimal: string;
  hex: string;
  binary: string;
  ipv6Mapped: string;
  ipv6Full: string;
}

export function getIpv4Conversions(ip: string): Ipv4Conversions | null {
  if (!isValidIpv4(ip)) return null;
  const intVal = ipv4ToInt(ip);
  return {
    decimal: intVal.toString(),
    hex: "0x" + intVal.toString(16).toUpperCase().padStart(8, "0"),
    binary: ipv4ToBinary(ip),
    ipv6Mapped: `::ffff:${ip}`,
    ipv6Full: ipv4ToHex(ip),
  };
}

export interface Ipv6Conversions {
  expanded: string;
  compressed: string;
  decimal: string;
}

export function getIpv6Conversions(ip: string): Ipv6Conversions | null {
  if (!isValidIpv6(ip)) return null;
  const expanded = expandIpv6(ip)!;
  const groups = expanded.split(":");
  const compressed = compressIpv6(groups);
  const decimal = groups
    .map((g) => BigInt("0x" + g))
    .reduce((acc, val, i) => acc + val * 65536n ** BigInt(7 - i), 0n)
    .toString();
  return { expanded, compressed, decimal };
}

function compressIpv6(groups: string[]): string {
  const nums = groups.map((g) => parseInt(g, 16));
  let bestStart = -1;
  let bestLen = 0;
  let curStart = -1;
  let curLen = 0;
  for (let i = 0; i < 8; i++) {
    if (nums[i] === 0) {
      if (curStart === -1) curStart = i;
      curLen = i - curStart + 1;
      if (curLen > bestLen) {
        bestStart = curStart;
        bestLen = curLen;
      }
    } else {
      curStart = -1;
    }
  }
  if (bestLen < 2)
    return groups.map((g) => g.replace(/^0+/, "") || "0").join(":");
  const left = groups
    .slice(0, bestStart)
    .map((g) => g.replace(/^0+/, "") || "0");
  const right = groups
    .slice(bestStart + bestLen)
    .map((g) => g.replace(/^0+/, "") || "0");
  if (left.length === 0 && right.length === 0) return "::";
  if (left.length === 0) return `::${right.join(":")}`;
  if (right.length === 0) return `${left.join(":")}::`;
  return `${left.join(":")}::${right.join(":")}`;
}

export interface SubnetInfo {
  networkAddress: string;
  broadcastAddress: string;
  subnetMask: string;
  subnetMaskBinary: string;
  wildcardMask: string;
  cidr: number;
  firstHost: string;
  lastHost: string;
  totalHosts: number;
  usableHosts: number;
  ipClass: string;
}

export function getSubnetInfo(ip: string, cidr: number): SubnetInfo | null {
  if (!isValidIpv4(ip)) return null;
  if (cidr < 0 || cidr > 32) return null;

  const ipInt = ipv4ToInt(ip);
  const maskInt =
    cidr === 0 ? 0n : (0xffffffffn << BigInt(32 - cidr)) & 0xffffffffn;
  const wildcardInt = ~maskInt & 0xffffffffn;
  const networkInt = ipInt & maskInt;
  const broadcastInt = networkInt | wildcardInt;

  const totalHosts = Number(wildcardInt) + 1;
  const usableHosts = cidr <= 30 ? totalHosts - 2 : cidr === 31 ? 2 : 1;

  const firstHost =
    cidr <= 30 ? intToIpv4(networkInt + 1n) : intToIpv4(networkInt);
  const lastHost =
    cidr <= 30 ? intToIpv4(broadcastInt - 1n) : intToIpv4(broadcastInt);

  const maskBinary = maskInt
    .toString(2)
    .padStart(32, "0")
    .match(/.{8}/g)!
    .join(".");

  const firstOctet = Number((ipInt >> 24n) & 255n);
  let ipClass: string;
  if (firstOctet < 128) ipClass = "A";
  else if (firstOctet < 192) ipClass = "B";
  else if (firstOctet < 224) ipClass = "C";
  else if (firstOctet < 240) ipClass = "D";
  else ipClass = "E";

  return {
    networkAddress: intToIpv4(networkInt),
    broadcastAddress: intToIpv4(broadcastInt),
    subnetMask: intToIpv4(maskInt),
    subnetMaskBinary: maskBinary,
    wildcardMask: intToIpv4(wildcardInt),
    cidr,
    firstHost,
    lastHost,
    totalHosts,
    usableHosts,
    ipClass,
  };
}
