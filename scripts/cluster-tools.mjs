// Tool similarity graph → TF-IDF weighted Jaccard → graphology Louvain → category assignment
// Cluster TOOLS directly (not keywords) for clean category boundaries
// Usage: node scripts/cluster-tools.mjs

import Graph from "graphology";
import louvain from "graphology-communities-louvain";

// ============================================================
// Tool definitions with enriched keywords
// Each keyword should describe: what it does, what domain, what format/standard
// TF-IDF will automatically downweight high-DF domain hints (dev, developer, coding, etc.)
// so they no longer dominate similarity — no manual separation needed
// ============================================================
const TOOLS = [
  {
    id: "hash-text",
    name: "Hash 计算",
    desc: "计算文本的 SHA1/SHA256/SHA512 哈希值",
    keywords: [
      "hash",
      "digest",
      "crypto",
      "sha1",
      "sha256",
      "sha512",
      "checksum",
      "integrity",
      "fingerprint",
      "one-way",
      "verify",
      "audit",
      "security",
      "secure",
    ],
  },
  {
    id: "uuid-generator",
    name: "UUID 生成",
    desc: "生成 v1/v4 UUID，支持批量生成",
    keywords: [
      "uuid",
      "guid",
      "v4",
      "v1",
      "unique-id",
      "random-id",
      "identifier",
      "rfc4122",
      "id-gen",
      "generate",
      "dev",
    ],
  },
  {
    id: "base64-string-converter",
    name: "Base64 编解码",
    desc: "字符串与 Base64 互转",
    keywords: [
      "base64",
      "encode",
      "decode",
      "binary-to-text",
      "rfc4648",
      "charset",
      "padding",
      "ascii",
      "transform",
      "converter",
    ],
  },
  {
    id: "color-converter",
    name: "颜色转换",
    desc: "HEX/RGB/HSL/RGBA 颜色格式互转",
    keywords: [
      "color",
      "colorspace",
      "rgb",
      "hsl",
      "rgba",
      "hex",
      "picker",
      "palette",
      "css-color",
      "design",
      "transform",
      "converter",
    ],
  },
  {
    id: "date-time-converter",
    name: "时间戳转换",
    desc: "Unix 时间戳与可读日期互转",
    keywords: [
      "date",
      "time",
      "timestamp",
      "unix",
      "epoch",
      "datetime",
      "calendar",
      "iso8601",
      "timezone",
      "locale",
      "transform",
      "converter",
    ],
  },
  {
    id: "integer-base-converter",
    name: "进制转换",
    desc: "二进制/八进制/十进制/十六进制互转",
    keywords: [
      "integer",
      "base",
      "binary",
      "octal",
      "decimal",
      "hex",
      "radix",
      "numeral",
      "bit",
      "digit",
      "transform",
      "converter",
    ],
  },
  {
    id: "url-encoder",
    name: "URL 编解码",
    desc: "URL encode/decode 与组件解析",
    keywords: [
      "url",
      "uri",
      "percent-encoding",
      "urlencode",
      "query-string",
      "path",
      "fragment",
      "rfc3986",
      "decode",
      "encode",
      "transform",
      "converter",
    ],
  },
  {
    id: "json-viewer",
    name: "JSON 格式化",
    desc: "JSON 格式化/压缩/验证与语法高亮",
    keywords: [
      "json",
      "code-format",
      "prettify",
      "minify",
      "validate",
      "viewer",
      "syntax",
      "highlight",
      "parse",
      "data-structure",
      "dev",
    ],
  },
  {
    id: "regex-tester",
    name: "正则测试器",
    desc: "测试正则表达式，匹配高亮与捕获组",
    keywords: [
      "regex",
      "regexp",
      "pattern",
      "match",
      "capture",
      "test",
      "flags",
      "group",
      "replace",
      "search",
      "dev",
    ],
  },
  {
    id: "jwt-parser",
    name: "JWT 解码",
    desc: "解码 JWT Token，展示 Header 与 Payload",
    keywords: [
      "jwt",
      "token",
      "claims",
      "issuer",
      "payload",
      "signature",
      "base64url",
      "jws",
      "authentication",
      "jwt-decode",
      "security",
      "crypto",
    ],
  },
  {
    id: "sql-formatter",
    name: "SQL 格式化",
    desc: "SQL 格式化/压缩，支持多种方言",
    keywords: [
      "sql",
      "code-format",
      "prettify",
      "minify",
      "dialect",
      "query",
      "database",
      "statement",
      "indent",
      "syntax",
      "dev",
    ],
  },
  {
    id: "cron-parser",
    name: "Cron 解析",
    desc: "Cron 表达式解析与下次执行时间",
    keywords: [
      "cron",
      "crontab",
      "schedule",
      "timer",
      "interval",
      "expression",
      "next-run",
      "periodic",
      "job",
      "dev",
      "time",
    ],
  },
  {
    id: "text-diff",
    name: "Diff 对比",
    desc: "双栏文本对比，差异高亮",
    keywords: [
      "diff",
      "compare",
      "difference",
      "highlight",
      "side-by-side",
      "patch",
      "change",
      "merge",
      "line-diff",
      "analyze",
      "dev",
    ],
  },
  {
    id: "text-statistics",
    name: "字数统计",
    desc: "统计字符数/单词数/行数/字节数",
    keywords: [
      "statistics",
      "count",
      "character",
      "word",
      "line",
      "byte",
      "analyze",
      "frequency",
      "str-length",
      "summary",
      "compare",
      "dev",
    ],
  },
  {
    id: "password-generator",
    name: "密码生成器",
    desc: "生成安全密码并评估强度与破解耗时",
    keywords: [
      "password",
      "strength",
      "entropy",
      "crack",
      "secure",
      "random",
      "brute-force",
      "policy",
      "uppercase",
      "symbol",
      "security",
      "crypto",
      "generate",
    ],
  },
  {
    id: "qrcode-generator",
    name: "二维码生成",
    desc: "文本/URL 生成二维码，支持颜色与容错等级",
    keywords: [
      "qrcode",
      "qr",
      "barcode",
      "matrix",
      "error-correction",
      "visual",
      "render",
      "download",
      "media",
      "graphic",
      "generate",
    ],
  },
  {
    id: "calculator",
    name: "计算器",
    desc: "数学表达式计算与历史记录",
    keywords: [
      "calculator",
      "math",
      "evaluate",
      "expression",
      "compute",
      "arithmetic",
      "operator",
      "parentheses",
      "history",
      "computation",
      "formula",
      "result",
      "numeric",
      "dev",
    ],
  },
  {
    id: "unit-converter",
    name: "单位换算",
    desc: "长度/重量/温度/面积/体积等互转",
    keywords: [
      "unit",
      "length",
      "weight",
      "temperature",
      "area",
      "volume",
      "measurement",
      "metric",
      "imperial",
      "si",
      "transform",
      "converter",
    ],
  },
  {
    id: "ip-info",
    name: "IP 查询",
    desc: "本机/指定 IP 地址信息查询与转换",
    keywords: [
      "ip",
      "address",
      "ipv4",
      "ipv6",
      "lookup",
      "geo",
      "geolocation",
      "subnet",
      "cidr",
      "host",
      "network",
      "dev",
      "dotted-decimal",
      "binary-addr",
    ],
  },
  {
    id: "image-compressor",
    name: "图片压缩",
    desc: "图片上传、质量/尺寸调整、压缩对比与下载",
    keywords: [
      "image",
      "compress",
      "resize",
      "quality",
      "optimize",
      "jpg",
      "png",
      "webp",
      "thumbnail",
      "download",
      "media",
      "graphic",
    ],
  },
  {
    id: "svg-preview",
    name: "SVG 预览",
    desc: "SVG 代码编辑、实时预览、优化与下载",
    keywords: [
      "svg",
      "preview",
      "edit",
      "optimize",
      "vector",
      "download",
      "graphics",
      "path",
      "xml",
      "render",
      "media",
      "graphic",
    ],
  },
  {
    id: "fs-link",
    name: "文件链接",
    desc: "创建符号链接/硬链接/目录联接",
    keywords: [
      "link",
      "symlink",
      "hardlink",
      "junction",
      "filesystem",
      "shortcut",
      "reference",
      "directory",
      "file",
      "mount",
      "path",
      "dev",
    ],
  },
];

// Display names for category labels (used only for labeling, NOT for clustering)
const LABEL_NAMES = {
  security: "安全",
  converter: "转换",
  dev: "开发",
  media: "媒体",
  network: "网络",
  filesystem: "文件系统",
  generate: "生成",
  design: "设计",
  format: "格式化",
};

// ============================================================
// Step 0: Compute IDF weights for all keywords
// IDF(kw) = log(N / DF(kw)) — high-DF words get low weight
// This automatically suppresses domain hints like "dev", "developer", "coding"
// ============================================================
const N = TOOLS.length;
const df = new Map();
const toolKwSets = TOOLS.map(
  (t) => new Set(t.keywords.map((k) => k.toLowerCase())),
);

for (const kwSet of toolKwSets) {
  for (const kw of kwSet) {
    df.set(kw, (df.get(kw) || 0) + 1);
  }
}

const idf = new Map();
for (const [kw, freq] of df) {
  idf.set(kw, Math.log(N / freq));
}

console.log(`=== IDF WEIGHT ANALYSIS ===\n`);
console.log(`  Tools: ${N}, Unique keywords: ${df.size}\n`);

const dfSorted = [...df.entries()].sort((a, b) => b[1] - a[1]);
console.log("  High-DF keywords (will be suppressed by IDF):");
for (const [kw, freq] of dfSorted.filter(([, f]) => f >= 3)) {
  console.log(
    `    ${kw.padEnd(20)} DF=${freq}/${N}  IDF=${idf.get(kw).toFixed(3)}`,
  );
}
console.log("\n  Low-DF keywords (high discriminative power):");
for (const [kw] of dfSorted.filter(([, f]) => f === 1).slice(0, 15)) {
  console.log(`    ${kw.padEnd(20)} DF=1/${N}  IDF=${idf.get(kw).toFixed(3)}`);
}

// ============================================================
// Step 1: Build tool-tool similarity graph with TF-IDF weighted Jaccard
// Weighted Jaccard: sum of IDF(kw) for kw in intersection / sum for union
// ============================================================
console.log(`\n=== TOOL SIMILARITY GRAPH (TF-IDF weighted Jaccard) ===\n`);

const toolIdfSums = toolKwSets.map((kwSet) => {
  let sum = 0;
  for (const kw of kwSet) sum += idf.get(kw);
  return sum;
});

const graph = new Graph({ type: "undirected", multi: false });

for (let i = 0; i < N; i++) {
  graph.addNode(String(i));
}

let edgeCount = 0;

for (let i = 0; i < N; i++) {
  for (let j = i + 1; j < N; j++) {
    const setA = toolKwSets[i];
    const setB = toolKwSets[j];

    let interWeight = 0;
    for (const kw of setA) {
      if (setB.has(kw)) interWeight += idf.get(kw);
    }
    const unionWeight = toolIdfSums[i] + toolIdfSums[j] - interWeight;
    const jaccard = unionWeight > 0 ? interWeight / unionWeight : 0;

    if (jaccard > 0) {
      graph.addEdgeWithKey(`${i}|${j}`, String(i), String(j), {
        weight: jaccard,
      });
      edgeCount++;
    }
  }
}

console.log(`  Edges: ${edgeCount}`);
console.log(`  Avg degree: ${((2 * edgeCount) / N).toFixed(1)}`);

const pairs = [];
for (let i = 0; i < N; i++) {
  for (let j = i + 1; j < N; j++) {
    if (graph.hasEdge(String(i), String(j))) {
      const w = graph.getEdgeAttribute(String(i), String(j), "weight");
      pairs.push({ i, j, w });
    }
  }
}
pairs.sort((a, b) => b.w - a.w);
console.log(`\n  Top tool-tool similarity pairs:`);
for (const p of pairs.slice(0, 15)) {
  console.log(
    `    ${TOOLS[p.i].id.padEnd(28)} ↔ ${TOOLS[p.j].id.padEnd(28)} weight=${p.w.toFixed(3)}`,
  );
}

// ============================================================
// Step 2: Run Louvain community detection on tool graph
// Target: 4-6 categories
// ============================================================
console.log(`\n=== LOUVAIN COMMUNITY DETECTION (tool graph) ===\n`);

const TARGET_COMM_MIN = 4;
const TARGET_COMM_MAX = 6;
const RESOLUTIONS = Array.from({ length: 20 }, (_, i) => 0.3 + i * 0.15);
let bestResolution = 1.0;
let bestMapping = null;
let bestDetails = null;
let bestScore = -Infinity;

function computeDiscriminability(mapping) {
  const catOf = {};
  for (const [node, cat] of Object.entries(mapping)) {
    catOf[node] = cat;
  }
  let totalIntra = 0,
    intraN = 0;
  let totalInter = 0,
    interN = 0;
  for (let i = 0; i < N; i++) {
    for (let j = i + 1; j < N; j++) {
      const w = graph.hasEdge(String(i), String(j))
        ? graph.getEdgeAttribute(String(i), String(j), "weight")
        : 0;
      if (catOf[String(i)] === catOf[String(j)]) {
        totalIntra += w;
        intraN++;
      } else {
        totalInter += w;
        interN++;
      }
    }
  }
  const intraAvg = intraN > 0 ? totalIntra / intraN : 0;
  const interAvg = interN > 0 ? totalInter / interN : 0;
  return interAvg > 0 ? intraAvg / interAvg : intraAvg > 0 ? 999 : 0;
}

console.log("  Resolution sweep:\n");
console.log(
  `  ${"res".padEnd(6)} ${"comms".padEnd(7)} ${"modularity".padEnd(12)} ${"discrim".padEnd(8)} distribution`,
);
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
    const inRange =
      comms.size >= TARGET_COMM_MIN && comms.size <= TARGET_COMM_MAX;
    console.log(
      `  ${String(res.toFixed(2)).padEnd(6)} ${String(comms.size).padEnd(7)} ${mod.toFixed(4).padEnd(12)} ${discrim.toFixed(1).padEnd(8)} [${sizes.join(", ")}]${inRange ? " ✓" : ""}`,
    );

    if (!inRange) continue;

    const maxSz = Math.max(...sizes);
    const minSz = Math.min(...sizes);
    const sizeRatio = maxSz / Math.max(minSz, 1);
    const balanceBonus = sizeRatio <= 2 ? 2 : sizeRatio <= 3 ? 1 : 0;
    const singletonPenalty = sizes.filter((s) => s === 1).length * 5;
    const score = discrim + balanceBonus - singletonPenalty;
    if (score > bestScore) {
      bestScore = score;
      bestResolution = res;
      bestMapping = m;
      bestDetails = d;
    }
  } catch (e) {
    console.log(`  ${String(res.toFixed(2)).padEnd(6)} ERROR: ${e.message}`);
  }
}

if (!bestMapping) {
  console.error(
    "\n  ❌ No resolution produced ${TARGET_COMM_MIN}-${TARGET_COMM_MAX} communities. Aborting.",
  );
  process.exit(1);
}

console.log(`\n  → Using resolution=${bestResolution.toFixed(2)}\n`);

const mapping = bestMapping;

const commGroups = new Map();
for (let i = 0; i < N; i++) {
  const c = mapping[String(i)];
  if (!commGroups.has(c)) commGroups.set(c, []);
  commGroups.get(c).push(i);
}

const sortedCommunities = [...commGroups.entries()].sort(
  (a, b) => b[1].length - a[1].length,
);

console.log(`  Communities: ${sortedCommunities.length}`);
console.log(`  Modularity: ${bestDetails.modularity?.toFixed(4) ?? "N/A"}\n`);

for (const [cId, toolIndices] of sortedCommunities) {
  const names = toolIndices.map((i) => TOOLS[i].id);
  console.log(
    `  Community ${cId} (${toolIndices.length} tools): ${names.join(", ")}`,
  );
}

// ============================================================
// Step 3: Label each community — data-driven discriminative labeling
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
      const globalRate =
        (globalHintCounts.get(hint) || 0) / allToolIndices.length;
      const discriminability = globalRate > 0 ? inRate / globalRate : Infinity;
      const score = count * discriminability;
      return { hint, count, inRate, globalRate, discriminability, score };
    })
    .sort((a, b) => b.score - a.score);

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
    cId,
    key,
    cn,
    toolIndices: [...toolIndices],
    keywords: [...allKws],
    hintCounts: label.hintCounts,
  });
}

for (const cat of catEntries) {
  const hints = cat.hintCounts
    .map((h) => `${h.hint}×${h.count}(s=${h.score.toFixed(1)})`)
    .join(", ");
  console.log(
    `  ${cat.cn.padEnd(8)} [${cat.key.padEnd(12)}] (${cat.toolIndices.length} tools) hints: ${hints}`,
  );
  for (const tIdx of cat.toolIndices) {
    console.log(`    ${TOOLS[tIdx].id.padEnd(28)} ${TOOLS[tIdx].name}`);
  }
}

// ============================================================
// Step 4: Merge single-tool categories into nearest neighbor
// ============================================================
const MIN_TOOLS_PER_CAT = 2;

console.log(`\n=== MERGE SMALL CATEGORIES (size < ${MIN_TOOLS_PER_CAT}) ===\n`);

let merged = true;
while (merged) {
  merged = false;
  const smallIdx = catEntries.findIndex(
    (c) => c.toolIndices.length < MIN_TOOLS_PER_CAT,
  );
  if (smallIdx < 0) break;

  const small = catEntries[smallIdx];
  let bestTarget = -1;
  let bestMergeScore = -Infinity;

  for (let i = 0; i < catEntries.length; i++) {
    if (i === smallIdx) continue;
    if (catEntries[i].toolIndices.length < MIN_TOOLS_PER_CAT) continue;

    const target = catEntries[i];
    let score = 0;

    if (small.key.split("_")[0] === target.key.split("_")[0]) score += 100;

    for (const sIdx of small.toolIndices) {
      for (const tIdx of target.toolIndices) {
        if (graph.hasEdge(String(sIdx), String(tIdx))) {
          score +=
            graph.getEdgeAttribute(String(sIdx), String(tIdx), "weight") * 5;
        }
      }
    }

    const smallKwSet = new Set(small.keywords);
    const kwOverlap = target.keywords.filter((k) => smallKwSet.has(k)).length;
    score += kwOverlap;

    score -= target.toolIndices.length * 0.5;

    if (score > bestMergeScore) {
      bestMergeScore = score;
      bestTarget = i;
    }
  }

  if (bestTarget >= 0) {
    const target = catEntries[bestTarget];
    console.log(
      `  MERGE: ${small.key} (${small.toolIndices.length} tools) → ${target.key} (${target.toolIndices.length} tools) [score=${bestMergeScore.toFixed(1)}]`,
    );
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
  const otherKws = catEntries
    .filter((c) => c !== cat)
    .flatMap((c) => c.keywords);
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
// Coverage validation
// ============================================================
const assignedIds = new Set(Object.keys(toolCatMap));
let coverageOk = true;
for (const t of TOOLS) {
  if (!assignedIds.has(t.id)) {
    console.error(`  ❌ Tool "${t.id}" not assigned to any category!`);
    coverageOk = false;
  }
}
if (assignedIds.size !== TOOLS.length) {
  console.error(
    `  ❌ Assigned ${assignedIds.size} tools but expected ${TOOLS.length}`,
  );
  coverageOk = false;
}
if (coverageOk) {
  console.log(
    `  ✓ All ${TOOLS.length} tools covered, ${catEntries.length} categories\n`,
  );
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
console.log(
  `  Mean: ${mean.toFixed(1)}, StdDev: ${Math.sqrt(variance).toFixed(1)}, CV: ${cv.toFixed(2)}`,
);
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

let totalIntra = 0,
  totalInter = 0,
  intraCount = 0,
  interCount = 0;
const toolDiscriminability = [];

for (let i = 0; i < N; i++) {
  const myCat = toolCatIndex.get(i);
  let intraSum = 0,
    intraN = 0;
  let interSum = 0,
    interN = 0;

  for (let j = 0; j < N; j++) {
    if (i === j) continue;
    const w = graph.hasEdge(String(i), String(j))
      ? graph.getEdgeAttribute(String(i), String(j), "weight")
      : 0;
    if (toolCatIndex.get(j) === myCat) {
      intraSum += w;
      intraN++;
    } else {
      interSum += w;
      interN++;
    }
  }

  const intraAvg = intraN > 0 ? intraSum / intraN : 0;
  const interAvg = interN > 0 ? interSum / interN : 0;
  const ratio = interAvg > 0 ? intraAvg / interAvg : Infinity;

  totalIntra += intraSum;
  intraCount += intraN;
  totalInter += interSum;
  interCount += interN;

  toolDiscriminability.push({
    id: TOOLS[i].id,
    cat: myCat,
    intraAvg,
    interAvg,
    ratio,
  });
}

toolDiscriminability.sort((a, b) => a.ratio - b.ratio);

console.log(
  `  ${"Tool".padEnd(28)} ${"Category".padEnd(12)} ${"Intra".padEnd(8)} ${"Inter".padEnd(8)} ${"Ratio".padEnd(8)}`,
);
console.log(`  ${"─".repeat(66)}`);
for (const t of toolDiscriminability) {
  const flag = t.ratio < 1.5 ? " ⚠" : "";
  console.log(
    `  ${t.id.padEnd(28)} ${t.cat.padEnd(12)} ${t.intraAvg.toFixed(3).padEnd(8)} ${t.interAvg.toFixed(3).padEnd(8)} ${t.ratio === Infinity ? "∞" : t.ratio.toFixed(2).padEnd(8)}${flag}`,
  );
}

const globalIntra = intraCount > 0 ? totalIntra / intraCount : 0;
const globalInter = interCount > 0 ? totalInter / interCount : 0;
const globalRatio = globalInter > 0 ? globalIntra / globalInter : Infinity;
const lowRatioCount = toolDiscriminability.filter((t) => t.ratio < 1.5).length;
console.log(
  `\n  Global intra-avg: ${globalIntra.toFixed(3)}, inter-avg: ${globalInter.toFixed(3)}, ratio: ${globalRatio === Infinity ? "∞" : globalRatio.toFixed(2)}`,
);
console.log(
  `  Low discriminability tools (ratio < 1.5): ${lowRatioCount}/${TOOLS.length}`,
);

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
  const row = catKeys
    .map((k2) => {
      const v = overlapMatrix[k1][k2];
      return k1 === k2
        ? "—".padEnd(10)
        : (v > 0 ? v.toFixed(2) : "0").padEnd(10);
    })
    .join("");
  console.log(`  ${k1.padEnd(12)}${row}`);
}

const totalOverlap = catKeys.reduce((sum, k1) => {
  return (
    sum +
    catKeys.reduce((s, k2) => (k1 === k2 ? s : s + overlapMatrix[k1][k2]), 0)
  );
}, 0);
console.log(
  `\n  Total cross-category overlap: ${totalOverlap.toFixed(2)} (lower is better)`,
);

// ============================================================
// Output: Expected Search Cost (ESC) — First Impression Model
// User sees tool name → first impression tells them a category
// If correct: scan = size of that category
// If wrong: scan = size(wrong) + size(actual) (user scans wrong cat, then correct cat)
// ESC = average scan count across all tools
// ============================================================
console.log(`\n${"=".repeat(70)}`);
console.log("=== EXPECTED SEARCH COST (ESC) — First Impression Model ===");
console.log(`${"=".repeat(70)}\n`);

const FIRST_IMPRESSION = {
  "hash-text": "security",
  "uuid-generator": "dev",
  "base64-string-converter": "converter",
  "color-converter": "converter",
  "date-time-converter": "converter",
  "integer-base-converter": "converter",
  "url-encoder": "converter",
  "json-viewer": "dev",
  "regex-tester": "dev",
  "jwt-parser": "security",
  "sql-formatter": "dev",
  "cron-parser": "dev",
  "text-diff": "dev",
  "text-statistics": "dev",
  "password-generator": "security",
  "qrcode-generator": "media",
  calculator: "dev",
  "unit-converter": "converter",
  "ip-info": "dev",
  "image-compressor": "media",
  "svg-preview": "media",
  "fs-link": "dev",
};

const catSizeMap = {};
for (const cat of catEntries) {
  catSizeMap[cat.key] = cat.toolIndices.length;
}

const escResults = [];
for (let i = 0; i < N; i++) {
  const toolId = TOOLS[i].id;
  const actualCat = toolCatMap[toolId];
  const impressionCat = FIRST_IMPRESSION[toolId];

  const isHit = impressionCat === actualCat;

  let scanCount;
  let foundAt;
  if (isHit) {
    scanCount = catSizeMap[actualCat];
    foundAt = 1;
  } else {
    scanCount = catSizeMap[impressionCat] + catSizeMap[actualCat];
    foundAt = 2;
  }

  escResults.push({
    id: toolId,
    actualCat,
    impressionCat,
    foundAt,
    scanCount,
    isHit,
  });
}

escResults.sort((a, b) => a.scanCount - b.scanCount);

const totalScan = escResults.reduce((s, r) => s + r.scanCount, 0);
const esc = totalScan / N;
const hitRate = escResults.filter((r) => r.isHit).length / N;
const baselineScan = N;
const improvement = (((baselineScan - esc) / baselineScan) * 100).toFixed(1);

console.log(
  `  ${"Tool".padEnd(28)} ${"Actual".padEnd(10)} ${"Impression".padEnd(12)} ${"Hit".padEnd(5)} ${"Scan".padEnd(6)}`,
);
console.log(`  ${"─".repeat(65)}`);
for (const r of escResults) {
  const flag = !r.isHit ? " ⚠" : "";
  console.log(
    `  ${r.id.padEnd(28)} ${r.actualCat.padEnd(10)} ${r.impressionCat.padEnd(12)} ${String(r.isHit ? "✓" : "✗").padEnd(5)} ${String(r.scanCount).padEnd(6)}${flag}`,
  );
}

console.log(
  `\n  ESC (Expected Search Cost):  ${esc.toFixed(1)} tools scanned on average`,
);
console.log(
  `  Baseline (no categories):     ${baselineScan} tools scanned (flat list, avg=N/2=11)`,
);
console.log(`  Search cost reduction:        ${improvement}%`);
console.log(
  `  First-choice hit rate:        ${(hitRate * 100).toFixed(1)}% (${escResults.filter((r) => r.isHit).length}/${N})`,
);

const missTools = escResults.filter((r) => !r.isHit);
if (missTools.length > 0) {
  console.log(`\n  Missed tools (first impression wrong):`);
  for (const r of missTools) {
    console.log(
      `    ${r.id} → impression:${r.impressionCat}, actual:${r.actualCat}, scan:${r.scanCount}`,
    );
  }
}

console.log(`\n// ESC formula: user first impression → category,`);
console.log(
  `// if correct scan=catSize, if wrong scan=wrongSize+actualSize, average over all tools.`,
);

console.log(`\n${"=".repeat(70)}`);
console.log("=== TYPESCRIPT OUTPUT ===");
console.log(`${"=".repeat(70)}\n`);

console.log(
  "// Auto-derived categories from tool similarity + Louvain (graphology):",
);
console.log("export const CATEGORY_KEYS = [");
for (const cat of catEntries) {
  console.log(`  "${cat.key}",`);
}
console.log("] as const;\n");
console.log("export type ToolCategory = (typeof CATEGORY_KEYS)[number];\n");

console.log(
  "const CATEGORY_SIGNATURES: Record<ToolCategory, readonly string[]> = {",
);
for (const cat of catEntries) {
  const sig = categorySignatures[cat.key];
  console.log(`  ${cat.key}: [${sig.map((k) => `"${k}"`).join(", ")}],`);
}
console.log("};\n");

console.log("// Tool → Category assignments:");
for (const [toolId, cat] of Object.entries(toolCatMap)) {
  console.log(`// ${toolId.padEnd(28)} → "${cat}"`);
}
