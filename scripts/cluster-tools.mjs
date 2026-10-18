// Tool similarity graph → graphology Louvain → category assignment
// Cluster TOOLS directly (not keywords) for clean category boundaries
// Usage: node scripts/cluster-tools.mjs

import Graph from "graphology";
import louvain from "graphology-communities-louvain";

// ============================================================
// Tool definitions with enriched keywords
// Each keyword should describe: what it does, what domain, what format/standard
// Domain hint keywords (last few) are shared within same-category tools
// ============================================================
const TOOLS = [
  {
    id: "hash-text",
    name: "Hash 计算",
    desc: "计算文本的 SHA1/SHA256/SHA512 哈希值",
    keywords: ["hash", "digest", "crypto", "sha1", "sha256", "sha512", "checksum", "integrity", "fingerprint", "one-way", "verify", "audit", "security", "secure"],
  },
  {
    id: "uuid-generator",
    name: "UUID 生成",
    desc: "生成 v1/v4 UUID，支持批量生成",
    keywords: ["uuid", "guid", "v4", "v1", "unique-id", "random-id", "identifier", "rfc4122", "id-gen", "dev", "developer", "coding"],
  },
  {
    id: "base64-string-converter",
    name: "Base64 编解码",
    desc: "字符串与 Base64 互转",
    keywords: ["devkit", "base64", "encode", "decode", "convert", "binary-to-text", "rfc4648", "charset", "padding", "ascii", "transform", "utility", "helper"],
  },
  {
    id: "color-converter",
    name: "颜色转换",
    desc: "HEX/RGB/HSL/RGBA 颜色格式互转",
    keywords: ["color", "colorspace", "rgb", "hsl", "rgba", "hex", "picker", "palette", "css-color", "design", "format", "converter"],
  },
  {
    id: "date-time-converter",
    name: "时间戳转换",
    desc: "Unix 时间戳与可读日期互转",
    keywords: ["date", "time", "timestamp", "unix", "epoch", "datetime", "calendar", "iso8601", "timezone", "locale", "format", "converter"],
  },
  {
    id: "integer-base-converter",
    name: "进制转换",
    desc: "二进制/八进制/十进制/十六进制互转",
    keywords: ["integer", "base", "binary", "octal", "decimal", "hex", "radix", "numeral", "bit", "digit", "format", "converter"],
  },
  {
    id: "url-encoder",
    name: "URL 编解码",
    desc: "URL encode/decode 与组件解析",
    keywords: ["devkit", "url", "uri", "percent-encoding", "urlencode", "query-string", "path", "fragment", "rfc3986", "decode", "encode", "protocol", "http", "web", "network", "internet", "utility", "helper"],
  },
  {
    id: "json-viewer",
    name: "JSON 格式化",
    desc: "JSON 格式化/压缩/验证与语法高亮",
    keywords: ["json", "code-format", "prettify", "minify", "validate", "viewer", "syntax", "highlight", "parse", "data-structure", "inspect", "dev", "developer", "coding"],
  },
  {
    id: "regex-tester",
    name: "正则测试器",
    desc: "测试正则表达式，匹配高亮与捕获组",
    keywords: ["regex", "regexp", "pattern", "match", "capture", "test", "flags", "group", "replace", "search", "inspect", "dev", "developer", "coding"],
  },
  {
    id: "jwt-parser",
    name: "JWT 解码",
    desc: "解码 JWT Token，展示 Header 与 Payload",
    keywords: ["jwt", "token", "claims", "issuer", "payload", "signature", "base64url", "jws", "authentication", "jwt-decode", "inspect", "security", "secure", "crypto"],
  },
  {
    id: "sql-formatter",
    name: "SQL 格式化",
    desc: "SQL 格式化/压缩，支持多种方言",
    keywords: ["sql", "code-format", "prettify", "minify", "dialect", "query", "database", "statement", "indent", "syntax", "dev", "developer", "coding"],
  },
  {
    id: "cron-parser",
    name: "Cron 解析",
    desc: "Cron 表达式解析与下次执行时间",
    keywords: ["cron", "crontab", "schedule", "timer", "interval", "expression", "next-run", "periodic", "job", "dev", "developer", "coding"],
  },
  {
    id: "text-diff",
    name: "Diff 对比",
    desc: "双栏文本对比，差异高亮",
    keywords: ["diff", "compare", "difference", "highlight", "side-by-side", "patch", "change", "merge", "line-diff", "text"],
  },
  {
    id: "text-statistics",
    name: "字数统计",
    desc: "统计字符数/单词数/行数/字节数",
    keywords: ["statistics", "count", "character", "word", "line", "byte", "analyze", "frequency", "str-length", "summary", "text"],
  },
  {
    id: "password-generator",
    name: "密码生成器",
    desc: "生成安全密码并评估强度与破解耗时",
    keywords: ["password", "strength", "entropy", "crack", "secure", "random", "brute-force", "policy", "uppercase", "symbol", "security", "crypto"],
  },
  {
    id: "qrcode-generator",
    name: "二维码生成",
    desc: "文本/URL 生成二维码，支持颜色与容错等级",
    keywords: ["qrcode", "qr", "barcode", "matrix", "error-correction", "visual", "render", "download", "media", "visual-media", "graphic"],
  },
  {
    id: "calculator",
    name: "计算器",
    desc: "数学表达式计算与历史记录",
    keywords: ["calculator", "math", "evaluate", "expression", "compute", "arithmetic", "operator", "parentheses", "history", "computation", "formula", "result", "numeric", "dev", "developer", "utility"],
  },
  {
    id: "unit-converter",
    name: "单位换算",
    desc: "长度/重量/温度/面积/体积等互转",
    keywords: ["unit", "length", "weight", "temperature", "area", "volume", "measurement", "metric", "imperial", "si", "converter", "math"],
  },
  {
    id: "ip-info",
    name: "IP 查询",
    desc: "本机/指定 IP 地址信息查询与转换",
    keywords: ["devkit", "ip", "address", "ipv4", "ipv6", "lookup", "geo", "geolocation", "subnet", "cidr", "host", "internet", "network", "networking", "utility", "helper"],
  },
  {
    id: "image-compressor",
    name: "图片压缩",
    desc: "图片上传、质量/尺寸调整、压缩对比与下载",
    keywords: ["image", "compress", "resize", "quality", "optimize", "jpg", "png", "webp", "thumbnail", "download", "media", "visual-media", "graphic"],
  },
  {
    id: "svg-preview",
    name: "SVG 预览",
    desc: "SVG 代码编辑、实时预览、优化与下载",
    keywords: ["svg", "preview", "edit", "optimize", "vector", "download", "graphics", "path", "xml", "render", "media", "visual-media", "graphic"],
  },
  {
    id: "fs-link",
    name: "文件链接",
    desc: "创建符号链接/硬链接/目录联接",
    keywords: ["link", "symlink", "hardlink", "junction", "filesystem", "shortcut", "reference", "directory", "file", "system", "mount", "os", "shell", "path", "node", "dev", "developer", "coding"],
  },
];

// Display names for common keywords (used only for labeling, NOT for clustering)
const LABEL_NAMES = {
  security: "安全", converter: "转换", dev: "开发", media: "媒体",
  text: "文本", network: "网络", networking: "网络", math: "数学", mathematics: "数学",
  filesystem: "文件系统", codec: "编解码", format: "格式化",
  generate: "生成", analyze: "分析", system: "系统",
};

// ============================================================
// Step 1: Build tool-tool similarity graph (PURE JACCARD, zero hardcoding)
// ============================================================
console.log(`=== TOOL SIMILARITY GRAPH ===\n`);
console.log(`  Tools: ${TOOLS.length}\n`);

const toolKwSets = TOOLS.map((t) => new Set(t.keywords.map((k) => k.toLowerCase())));

const graph = new Graph({ type: "undirected", multi: false });

for (let i = 0; i < TOOLS.length; i++) {
  graph.addNode(String(i));
}

let edgeCount = 0;

for (let i = 0; i < TOOLS.length; i++) {
  for (let j = i + 1; j < TOOLS.length; j++) {
    const setA = toolKwSets[i];
    const setB = toolKwSets[j];

    let intersection = 0;
    for (const kw of setA) {
      if (setB.has(kw)) intersection++;
    }
    const union = setA.size + setB.size - intersection;
    const jaccard = union > 0 ? intersection / union : 0;

    if (jaccard > 0) {
      graph.addEdgeWithKey(`${i}|${j}`, String(i), String(j), { weight: jaccard });
      edgeCount++;
    }
  }
}

console.log(`  Edges: ${edgeCount}`);
console.log(`  Avg degree: ${(2 * edgeCount / TOOLS.length).toFixed(1)}`);

const pairs = [];
for (let i = 0; i < TOOLS.length; i++) {
  for (let j = i + 1; j < TOOLS.length; j++) {
    if (graph.hasEdge(String(i), String(j))) {
      const w = graph.getEdgeAttribute(String(i), String(j), "weight");
      pairs.push({ i, j, w });
    }
  }
}
pairs.sort((a, b) => b.w - a.w);
console.log(`\n  Top tool-tool similarity pairs:`);
for (const p of pairs.slice(0, 12)) {
  console.log(`    ${TOOLS[p.i].id.padEnd(28)} ↔ ${TOOLS[p.j].id.padEnd(28)} weight=${p.w.toFixed(3)}`);
}

// ============================================================
// Step 2: Run Louvain community detection on tool graph
// ============================================================
console.log(`\n=== LOUVAIN COMMUNITY DETECTION (tool graph) ===\n`);

const RESOLUTIONS = [0.5, 0.8, 1.0, 1.2, 1.5, 1.8, 2.0, 2.5, 3.0];
const TARGET_COMM_MIN = 5;
const TARGET_COMM_MAX = 9;
let bestResolution = 1.0;
let bestMapping = null;
let bestDetails = null;
let bestScore = -Infinity;

function computeDiscriminability(mapping) {
  const catOf = {};
  for (const [node, cat] of Object.entries(mapping)) {
    catOf[node] = cat;
  }
  let totalIntra = 0, intraN = 0;
  let totalInter = 0, interN = 0;
  for (let i = 0; i < TOOLS.length; i++) {
    for (let j = i + 1; j < TOOLS.length; j++) {
      const w = graph.hasEdge(String(i), String(j)) ? graph.getEdgeAttribute(String(i), String(j), "weight") : 0;
      if (catOf[String(i)] === catOf[String(j)]) {
        totalIntra += w; intraN++;
      } else {
        totalInter += w; interN++;
      }
    }
  }
  const intraAvg = intraN > 0 ? totalIntra / intraN : 0;
  const interAvg = interN > 0 ? totalInter / interN : 0;
  return interAvg > 0 ? intraAvg / interAvg : (intraAvg > 0 ? 999 : 0);
}

console.log("  Resolution sweep:\n");
console.log(`  ${"res".padEnd(6)} ${"comms".padEnd(7)} ${"modularity".padEnd(12)} ${"discrim".padEnd(8)} distribution`);
console.log(`  ${"─".repeat(65)}`);

for (const res of RESOLUTIONS) {
  try {
    const m = louvain(graph, { resolution: res });
    const d = louvain.detailed(graph, { resolution: res });
    const comms = new Set(Object.values(m));
    const commSizes = new Map();
    for (const c of Object.values(m)) {
      commSizes.set(c, (commSizes.get(c) || 0) + 1);
    }
    const sizes = [...commSizes.values()].sort((a, b) => b - a);
    const mod = d.modularity ?? 0;
    const discrim = computeDiscriminability(m);
    console.log(`  ${String(res).padEnd(6)} ${String(comms.size).padEnd(7)} ${mod.toFixed(4).padEnd(12)} ${discrim.toFixed(1).padEnd(8)} [${sizes.join(", ")}]`);

    if (comms.size < TARGET_COMM_MIN || comms.size > TARGET_COMM_MAX) continue;

    const maxSz = Math.max(...sizes);
    const minSz = Math.min(...sizes);
    const sizeRatio = maxSz / Math.max(minSz, 1);
    const balanceBonus = sizeRatio <= 2 ? 2 : sizeRatio <= 3 ? 1 : 0;
    const singletonPenalty = sizes.filter((s) => s === 1).length * 3;
    const score = discrim + balanceBonus - singletonPenalty;
    if (score > bestScore) {
      bestScore = score;
      bestResolution = res;
      bestMapping = m;
      bestDetails = d;
    }
  } catch (e) {
    console.log(`  ${String(res).padEnd(6)} ERROR: ${e.message}`);
  }
}

console.log(`\n  → Using resolution=${bestResolution}\n`);

const mapping = bestMapping;

const commGroups = new Map();
for (let i = 0; i < TOOLS.length; i++) {
  const c = mapping[String(i)];
  if (!commGroups.has(c)) commGroups.set(c, []);
  commGroups.get(c).push(i);
}

const sortedCommunities = [...commGroups.entries()]
  .sort((a, b) => b[1].length - a[1].length);

console.log(`  Communities: ${sortedCommunities.length}`);
console.log(`  Modularity: ${bestDetails.modularity?.toFixed(4) ?? "N/A"}\n`);

for (const [cId, toolIndices] of sortedCommunities) {
  const names = toolIndices.map((i) => TOOLS[i].id);
  console.log(`  Community ${cId} (${toolIndices.length} tools): ${names.join(", ")}`);
}

// ============================================================
// Step 3: Label each community — DATA-DRIVEN, not hardcoded
// Count domain hint occurrences, pick the dominant one
// Fallback: derive label from most discriminative keyword
// ============================================================
console.log(`\n=== LABEL COMMUNITIES (data-driven) ===\n`);

function labelCommunity(toolIndices, allToolIndices) {
  const knownHints = Object.keys(LABEL_NAMES);
  const hintCounts = new Map();
  for (const tIdx of toolIndices) {
    for (const hint of knownHints) {
      if (toolKwSets[tIdx].has(hint)) {
        hintCounts.set(hint, (hintCounts.get(hint) || 0) + 1);
      }
    }
  }

  const communitySize = toolIndices.length;

  const globalHintCounts = new Map();
  for (const tIdx of allToolIndices) {
    for (const hint of knownHints) {
      if (toolKwSets[tIdx].has(hint)) {
        globalHintCounts.set(hint, (globalHintCounts.get(hint) || 0) + 1);
      }
    }
  }

  const scoredHints = [...hintCounts.entries()]
    .map(([hint, count]) => {
      const inRate = count / communitySize;
      const globalRate = (globalHintCounts.get(hint) || 0) / allToolIndices.length;
      const discriminability = globalRate > 0 ? inRate / globalRate : Infinity;
      return { hint, count, inRate, globalRate, discriminability };
    })
    .sort((a, b) => b.discriminability - a.discriminability);

  if (scoredHints.length > 0 && scoredHints[0].count > 0) {
    const best = scoredHints[0];
    return {
      key: best.hint,
      cn: LABEL_NAMES[best.hint] ?? best.hint,
      hintCounts: scoredHints,
    };
  }

  const kwCounts = new Map();
  for (const tIdx of toolIndices) {
    for (const kw of toolKwSets[tIdx]) {
      if (!knownHints.includes(kw)) {
        kwCounts.set(kw, (kwCounts.get(kw) || 0) + 1);
      }
    }
  }
  const topKw = [...kwCounts.entries()].sort((a, b) => b[1] - a[1])[0];
  return {
    key: topKw?.[0] ?? "misc",
    cn: topKw?.[0] ?? "其他",
    hintCounts: [],
  };
}

const catEntries = [];
const usedKeys = new Set();

const allToolIndices = TOOLS.map((_, i) => i);

for (const [cId, toolIndices] of sortedCommunities) {
  const label = labelCommunity(toolIndices, allToolIndices);
  let key = label.key;
  let cn = label.cn;

  if (usedKeys.has(key)) {
    let suffix = 2;
    while (usedKeys.has(`${key}_${suffix}`)) suffix++;
    key = `${key}_${suffix}`;
    cn = `${cn}${suffix}`;
  }
  usedKeys.add(key);

  const allKws = new Set();
  for (const tIdx of toolIndices) {
    for (const kw of toolKwSets[tIdx]) allKws.add(kw);
  }

  catEntries.push({
    cId, key, cn,
    toolIndices: [...toolIndices],
    keywords: [...allKws],
    hintCounts: label.hintCounts,
  });
}

for (const cat of catEntries) {
  const hints = cat.hintCounts.map((h) => `${h.hint}×${h.count}(d=${h.discriminability.toFixed(1)})`).join(", ");
  console.log(`  ${cat.cn.padEnd(8)} [${cat.key.padEnd(12)}] (${cat.toolIndices.length} tools) hints: ${hints}`);
  for (const tIdx of cat.toolIndices) {
    console.log(`    ${TOOLS[tIdx].id.padEnd(28)} ${TOOLS[tIdx].name}`);
  }
}

// ============================================================
// Step 4: Merge small categories (single-tool) into nearest neighbor
// ============================================================
const MIN_TOOLS_PER_CAT = 1;

console.log(`\n=== MERGE SMALL CATEGORIES ===\n`);

let merged = true;
while (merged) {
  merged = false;
  const smallIdx = catEntries.findIndex((c) => c.toolIndices.length < MIN_TOOLS_PER_CAT);
  if (smallIdx < 0) break;

  const small = catEntries[smallIdx];
  let bestTarget = -1;
  let bestScore = -Infinity;

  for (let i = 0; i < catEntries.length; i++) {
    if (i === smallIdx) continue;
    if (catEntries[i].toolIndices.length < MIN_TOOLS_PER_CAT) continue;

    const target = catEntries[i];
    let score = 0;

    // Domain hint match (same base key)
    if (small.key.split("_")[0] === target.key.split("_")[0]) score += 100;

    // Direct graph edge weight (tool-tool similarity)
    for (const sIdx of small.toolIndices) {
      for (const tIdx of target.toolIndices) {
        if (graph.hasEdge(String(sIdx), String(tIdx))) {
          score += graph.getEdgeAttribute(String(sIdx), String(tIdx), "weight") * 5;
        }
      }
    }

    // Keyword overlap
    const smallKwSet = new Set(small.keywords);
    const kwOverlap = target.keywords.filter((k) => smallKwSet.has(k)).length;
    score += kwOverlap;

    // Size penalty: prefer merging into smaller categories for balance
    score -= target.toolIndices.length * 0.5;

    if (score > bestScore) {
      bestScore = score;
      bestTarget = i;
    }
  }

  if (bestTarget >= 0) {
    const target = catEntries[bestTarget];
    console.log(`  MERGE: ${small.key} (${small.toolIndices.length} tools) → ${target.key} (${target.toolIndices.length} tools) [score=${bestScore.toFixed(1)}]`);
    target.toolIndices = [...target.toolIndices, ...small.toolIndices];
    target.keywords = [...new Set([...target.keywords, ...small.keywords])];
    target.key = target.key.split("_")[0];
    target.cn = LABEL_NAMES[target.key] ?? target.key;
    catEntries.splice(smallIdx, 1);
    merged = true;
  } else {
    break;
  }
}

const deduped = new Map();
for (const cat of catEntries) {
  if (deduped.has(cat.key)) {
    const existing = deduped.get(cat.key);
    existing.toolIndices = [...existing.toolIndices, ...cat.toolIndices];
    existing.keywords = [...new Set([...existing.keywords, ...cat.keywords])];
  } else {
    deduped.set(cat.key, cat);
  }
}
catEntries.length = 0;
catEntries.push(...deduped.values());
catEntries.sort((a, b) => b.toolIndices.length - a.toolIndices.length);

// ============================================================
// Step 5: Derive discriminative signatures per category
// ============================================================
function discriminativeSignature(catKeywords, otherKeywords) {
  const otherFreq = new Map();
  for (const kw of otherKeywords) {
    otherFreq.set(kw, (otherFreq.get(kw) || 0) + 1);
  }
  const scored = catKeywords.map((kw) => {
    const outFreq = otherFreq.get(kw) || 0;
    const score = 1 / (1 + outFreq * 2);
    return { kw, score };
  });
  scored.sort((a, b) => b.score - a.score);
  return scored.slice(0, 12).map((s) => s.kw);
}

// ============================================================
// Output: Final category structure
// ============================================================
console.log(`\n${"=".repeat(70)}`);
console.log("=== FINAL CATEGORY STRUCTURE ===");
console.log(`${"=".repeat(70)}\n`);

const categorySignatures = {};
const toolCatMap = {};

for (const cat of catEntries) {
  const otherKws = catEntries.filter((c) => c !== cat).flatMap((c) => c.keywords);
  const sig = discriminativeSignature(cat.keywords, otherKws);
  categorySignatures[cat.key] = sig;

  for (const tIdx of cat.toolIndices) {
    toolCatMap[TOOLS[tIdx].id] = cat.key;
  }

  console.log(`  ┌─ ${cat.cn} [${cat.key}] (${cat.toolIndices.length} tools)`);
  console.log(`  │  Signature: [${sig.map((k) => `"${k}"`).join(", ")}]`);
  for (const tIdx of cat.toolIndices) {
    console.log(`  │  ${TOOLS[tIdx].id.padEnd(28)} ${TOOLS[tIdx].name}`);
  }
  console.log(`  └─`);
  console.log();
}

// ============================================================
// Output: Distribution analysis
// ============================================================
console.log(`${"=".repeat(70)}`);
console.log("=== DISTRIBUTION ANALYSIS ===");
console.log(`${"=".repeat(70)}\n`);

const sizes = catEntries.map((c) => c.toolIndices.length);
const maxSz = Math.max(...sizes);
const minSz = Math.min(...sizes);
const total = sizes.reduce((a, b) => a + b, 0);
const mean = total / sizes.length;
const variance = sizes.reduce((a, s) => a + (s - mean) ** 2, 0) / sizes.length;
const cv = Math.sqrt(variance) / mean;

console.log(`  Categories: ${catEntries.length}`);
console.log(`  Tools: ${total}`);
console.log(`  Distribution: [${sizes.join(", ")}]`);
console.log(`  Mean: ${mean.toFixed(1)}, StdDev: ${Math.sqrt(variance).toFixed(1)}, CV: ${cv.toFixed(2)}`);
console.log(`  Max/Min ratio: ${(maxSz / minSz).toFixed(1)}\n`);

console.log("  Visual:");
for (const cat of catEntries) {
  const bar = "█".repeat(cat.toolIndices.length * 3);
  console.log(`  ${cat.key.padEnd(12)} ${bar} ${cat.toolIndices.length}`);
}

// ============================================================
// Output: Tool discriminability (intra vs inter similarity)
// ============================================================
console.log(`\n${"=".repeat(70)}`);
console.log("=== TOOL DISCRIMINABILITY ===");
console.log(`${"=".repeat(70)}\n`);

const toolCatIndex = new Map();
for (const cat of catEntries) {
  for (const tIdx of cat.toolIndices) {
    toolCatIndex.set(tIdx, cat.key);
  }
}

let totalIntra = 0, totalInter = 0, intraCount = 0, interCount = 0;
const toolDiscriminability = [];

for (let i = 0; i < TOOLS.length; i++) {
  const myCat = toolCatIndex.get(i);
  let intraSum = 0, intraN = 0;
  let interSum = 0, interN = 0;

  for (let j = 0; j < TOOLS.length; j++) {
    if (i === j) continue;
    const w = graph.hasEdge(String(i), String(j)) ? graph.getEdgeAttribute(String(i), String(j), "weight") : 0;
    if (toolCatIndex.get(j) === myCat) {
      intraSum += w; intraN++;
    } else {
      interSum += w; interN++;
    }
  }

  const intraAvg = intraN > 0 ? intraSum / intraN : 0;
  const interAvg = interN > 0 ? interSum / interN : 0;
  const ratio = interAvg > 0 ? intraAvg / interAvg : Infinity;

  totalIntra += intraSum; intraCount += intraN;
  totalInter += interSum; interCount += interN;

  toolDiscriminability.push({ id: TOOLS[i].id, cat: myCat, intraAvg, interAvg, ratio });
}

toolDiscriminability.sort((a, b) => a.ratio - b.ratio);

console.log(`  ${"Tool".padEnd(28)} ${"Category".padEnd(12)} ${"Intra".padEnd(8)} ${"Inter".padEnd(8)} ${"Ratio".padEnd(8)}`);
console.log(`  ${"─".repeat(66)}`);
for (const t of toolDiscriminability) {
  const flag = t.ratio < 1.5 ? " ⚠" : "";
  console.log(`  ${t.id.padEnd(28)} ${t.cat.padEnd(12)} ${t.intraAvg.toFixed(3).padEnd(8)} ${t.interAvg.toFixed(3).padEnd(8)} ${t.ratio === Infinity ? "∞" : t.ratio.toFixed(2).padEnd(8)}${flag}`);
}

const globalIntra = intraCount > 0 ? totalIntra / intraCount : 0;
const globalInter = interCount > 0 ? totalInter / interCount : 0;
const globalRatio = globalInter > 0 ? globalIntra / globalInter : Infinity;
const lowRatioCount = toolDiscriminability.filter((t) => t.ratio < 1.5).length;
console.log(`\n  Global intra-avg: ${globalIntra.toFixed(3)}, inter-avg: ${globalInter.toFixed(3)}, ratio: ${globalRatio === Infinity ? "∞" : globalRatio.toFixed(2)}`);
console.log(`  Low discriminability tools (ratio < 1.5): ${lowRatioCount}/${TOOLS.length}`);

// ============================================================
// Output: Cross-category keyword overlap
// ============================================================
console.log(`\n${"=".repeat(70)}`);
console.log("=== CROSS-CATEGORY KEYWORD OVERLAP ===");
console.log(`${"=".repeat(70)}\n`);

const catKeys = catEntries.map((c) => c.key);
const overlapMatrix = {};
for (const k1 of catKeys) {
  overlapMatrix[k1] = {};
  const s1 = new Set(categorySignatures[k1] || []);
  for (const k2 of catKeys) {
    const s2 = new Set(categorySignatures[k2] || []);
    let inter = 0;
    for (const k of s1) if (s2.has(k)) inter++;
    overlapMatrix[k1][k2] = s2.size > 0 ? inter / s2.size : 0;
  }
}

console.log(`  ${"".padEnd(12)}${catKeys.map((k) => k.padEnd(10)).join("")}`);
for (const k1 of catKeys) {
  const row = catKeys.map((k2) => {
    const v = overlapMatrix[k1][k2];
    return k1 === k2 ? "—".padEnd(10) : (v > 0 ? v.toFixed(2) : "0").padEnd(10);
  }).join("");
  console.log(`  ${k1.padEnd(12)}${row}`);
}

const totalOverlap = catKeys.reduce((sum, k1) => {
  return sum + catKeys.reduce((s, k2) => k1 === k2 ? s : s + overlapMatrix[k1][k2], 0);
}, 0);
console.log(`\n  Total cross-category overlap: ${totalOverlap.toFixed(2)} (lower is better)`);

// ============================================================
// Output: TypeScript code
// ============================================================
console.log(`\n${"=".repeat(70)}`);
console.log("=== TYPESCRIPT OUTPUT ===");
console.log(`${"=".repeat(70)}\n`);

console.log("// Auto-derived categories from tool similarity + Louvain (graphology):");
console.log("export const CATEGORY_KEYS = [");
for (const cat of catEntries) {
  console.log(`  "${cat.key}",`);
}
console.log("] as const;\n");
console.log("export type ToolCategory = (typeof CATEGORY_KEYS)[number];\n");

console.log("const CATEGORY_SIGNATURES: Record<ToolCategory, readonly string[]> = {");
for (const cat of catEntries) {
  const sig = categorySignatures[cat.key];
  console.log(`  ${cat.key}: [${sig.map((k) => `"${k}"`).join(", ")}],`);
}
console.log("};\n");

console.log("// Tool → Category assignments:");
for (const [toolId, cat] of Object.entries(toolCatMap)) {
  console.log(`// ${toolId.padEnd(28)} → "${cat}"`);
}