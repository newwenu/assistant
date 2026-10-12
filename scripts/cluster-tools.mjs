// Hierarchical Agglomerative Clustering (HAC) for tool category derivation
// Uses keyword co-occurrence embedding + description semantics for richer features
// Usage: node scripts/cluster-tools.mjs

const TOOLS = [
  { id: "hash-text", name: "Hash 计算", desc: "计算文本的 SHA1/SHA256/SHA512 哈希值", keywords: ["hash", "digest", "crypto", "SHA1", "SHA256", "SHA512"] },
  { id: "uuid-generator", name: "UUID 生成", desc: "生成 v1/v4 UUID，支持批量生成", keywords: ["uuid", "v4", "v1", "random", "id", "unique"] },
  { id: "base64-string-converter", name: "Base64 编解码", desc: "字符串与 Base64 互转", keywords: ["base64", "encode", "decode", "convert"] },
  { id: "color-converter", name: "颜色转换", desc: "HEX/RGB/HSL/RGBA 颜色格式互转", keywords: ["color", "hex", "rgb", "hsl", "rgba", "picker", "converter"] },
  { id: "date-time-converter", name: "时间戳转换", desc: "Unix 时间戳与可读日期互转", keywords: ["date", "time", "timestamp", "unix", "converter", "epoch"] },
  { id: "integer-base-converter", name: "进制转换", desc: "二进制/八进制/十进制/十六进制互转", keywords: ["integer", "base", "binary", "octal", "decimal", "hex", "convert"] },
  { id: "url-encoder", name: "URL 编解码", desc: "URL encode/decode 与组件解析", keywords: ["url", "encode", "decode", "uri", "component"] },
  { id: "json-viewer", name: "JSON 格式化", desc: "JSON 格式化/压缩/验证与语法高亮", keywords: ["json", "format", "prettify", "minify", "validate", "viewer"] },
  { id: "regex-tester", name: "正则测试器", desc: "测试正则表达式，匹配高亮与捕获组", keywords: ["regex", "regexp", "test", "match", "capture"] },
  { id: "jwt-parser", name: "JWT 解码", desc: "解码 JWT Token，展示 Header 与 Payload", keywords: ["jwt", "token", "decode", "parse", "header", "payload"] },
  { id: "sql-formatter", name: "SQL 格式化", desc: "SQL 格式化/压缩，支持多种方言", keywords: ["sql", "format", "prettify", "minify", "dialect", "query"] },
  { id: "cron-parser", name: "Cron 解析", desc: "Cron 表达式解析与下次执行时间", keywords: ["cron", "crontab", "schedule", "parse", "timer"] },
  { id: "text-diff", name: "Diff 对比", desc: "双栏文本对比，差异高亮", keywords: ["diff", "compare", "text", "difference"] },
  { id: "text-statistics", name: "字数统计", desc: "统计字符数/单词数/行数/字节数", keywords: ["text", "statistics", "count", "character", "word", "line", "byte"] },
  { id: "password-generator", name: "密码生成器", desc: "生成安全密码并评估强度与破解耗时", keywords: ["password", "generator", "strength", "entropy", "crack", "secure"] },
  { id: "qrcode-generator", name: "二维码生成", desc: "文本/URL 生成二维码，支持颜色与容错等级", keywords: ["qrcode", "qr", "code", "generate", "url", "text"] },
  { id: "calculator", name: "计算器", desc: "数学表达式计算与历史记录", keywords: ["calculator", "math", "evaluate", "expression", "compute"] },
  { id: "unit-converter", name: "单位换算", desc: "长度/重量/温度/面积/体积等互转", keywords: ["unit", "convert", "length", "weight", "temperature", "area", "volume"] },
  { id: "ip-info", name: "IP 查询", desc: "本机/指定 IP 地址信息查询与转换", keywords: ["ip", "address", "ipv4", "ipv6", "lookup", "geo"] },
  { id: "image-compressor", name: "图片压缩", desc: "图片上传、质量/尺寸调整、压缩对比与下载", keywords: ["image", "compress", "resize", "quality", "optimize", "jpg", "png"] },
  { id: "svg-preview", name: "SVG 预览", desc: "SVG 代码编辑、实时预览、优化与下载", keywords: ["svg", "preview", "edit", "optimize", "vector", "download"] },
];

// === Step 1: Build keyword co-occurrence graph ===
// Keywords that appear in the same tool are co-occurring
const allKeywords = new Set();
for (const t of TOOLS) {
  for (const kw of t.keywords) allKeywords.add(kw.toLowerCase());
}
const kwList = [...allKeywords].sort();
const kwIndex = new Map(kwList.map((kw, i) => [kw, i]));

// Co-occurrence matrix: how many tools contain both kw_i and kw_j
const cooc = Array.from({ length: kwList.length }, () => new Float64Array(kwList.length));
for (const t of TOOLS) {
  const kws = t.keywords.map((k) => k.toLowerCase());
  for (let i = 0; i < kws.length; i++) {
    for (let j = 0; j < kws.length; j++) {
      cooc[kwIndex.get(kws[i])][kwIndex.get(kws[j])] += 1;
    }
  }
}

// === Step 2: Keyword embedding via PMI (Pointwise Mutual Information) ===
// PMI(kw_i, kw_j) = log(P(kw_i, kw_j) / (P(kw_i) * P(kw_j)))
const N = TOOLS.length;
const kwMarginal = new Float64Array(kwList.length);
for (let i = 0; i < kwList.length; i++) {
  kwMarginal[i] = cooc[i][i] / N;
}

const pmi = Array.from({ length: kwList.length }, () => new Float64Array(kwList.length));
for (let i = 0; i < kwList.length; i++) {
  for (let j = 0; j < kwList.length; j++) {
    const pJoint = cooc[i][j] / N;
    const pIndep = kwMarginal[i] * kwMarginal[j];
    if (pJoint > 0 && pIndep > 0) {
      pmi[i][j] = Math.max(0, Math.log(pJoint / pIndep));
    }
  }
}

// === Step 3: Project each tool into PMI embedding space ===
// Tool vector = sum of PMI rows for its keywords (bag-of-keywords in PMI space)
function toolEmbedding(tool) {
  const vec = new Float64Array(kwList.length);
  for (const kw of tool.keywords) {
    const idx = kwIndex.get(kw.toLowerCase());
    if (idx !== undefined) {
      for (let j = 0; j < kwList.length; j++) {
        vec[j] += pmi[idx][j];
      }
    }
  }
  return vec;
}

const embeddings = TOOLS.map(toolEmbedding);

// === Step 4: Semantic feature dimensions ===
// These capture functional intent beyond keyword overlap
const SEMANTIC_DIMS = {
  codec:      ["encode", "decode", "convert", "converter", "format", "prettify", "minify", "parse", "compress"],
  security:   ["crypto", "hash", "digest", "secure", "password", "entropy", "crack", "strength", "jwt", "token"],
  dev:        ["json", "sql", "regex", "regexp", "cron", "crontab", "query", "dialect", "validate", "format", "prettify"],
  text:       ["text", "diff", "compare", "difference", "statistics", "count", "character", "word", "line", "byte"],
  media:      ["image", "svg", "qrcode", "qr", "compress", "resize", "optimize", "preview", "edit", "vector", "download"],
  generate:   ["generate", "generator", "uuid", "random", "create", "qrcode"],
  network:    ["ip", "url", "uri", "address", "lookup", "geo", "ipv4", "ipv6", "network"],
  math:       ["math", "calculator", "compute", "evaluate", "expression", "unit", "length", "weight", "temperature"],
  time:       ["date", "time", "timestamp", "epoch", "cron", "schedule", "timer", "crontab"],
};

function semanticVector(tool) {
  const kws = new Set(tool.keywords.map((k) => k.toLowerCase()));
  const vec = {};
  for (const [dim, dimKws] of Object.entries(SEMANTIC_DIMS)) {
    let score = 0;
    for (const dk of dimKws) {
      if (kws.has(dk)) score += 1;
    }
    vec[dim] = score / dimKws.length;
  }
  return vec;
}

const semVectors = TOOLS.map(semanticVector);

// === Step 5: Combined similarity ===
function cosineSim(a, b) {
  let dot = 0, normA = 0, normB = 0;
  for (let i = 0; i < a.length; i++) {
    dot += a[i] * b[i];
    normA += a[i] * a[i];
    normB += b[i] * b[i];
  }
  if (normA === 0 || normB === 0) return 0;
  return dot / (Math.sqrt(normA) * Math.sqrt(normB));
}

function semanticCosineSim(a, b) {
  const dims = Object.keys(a);
  let dot = 0, normA = 0, normB = 0;
  for (const d of dims) {
    dot += a[d] * b[d];
    normA += a[d] * a[d];
    normB += b[d] * b[d];
  }
  if (normA === 0 || normB === 0) return 0;
  return dot / (Math.sqrt(normA) * Math.sqrt(normB));
}

function combinedSimilarity(i, j) {
  const pmiSim = cosineSim(embeddings[i], embeddings[j]);
  const semSim = semanticCosineSim(semVectors[i], semVectors[j]);

  // Also add raw keyword Jaccard
  const kwsA = new Set(TOOLS[i].keywords.map((k) => k.toLowerCase()));
  const kwsB = new Set(TOOLS[j].keywords.map((k) => k.toLowerCase()));
  let inter = 0;
  for (const k of kwsA) if (kwsB.has(k)) inter++;
  const jaccard = inter / (kwsA.size + kwsB.size - inter);

  // Weighted combination: semantic is most important, then PMI, then raw Jaccard
  return 0.55 * semSim + 0.30 * pmiSim + 0.15 * jaccard;
}

// === Step 6: HAC with average linkage ===
function hac(items, simFn, targetClusters) {
  const n = items.length;
  let clusters = items.map((_, i) => ({ id: i, members: [i] }));

  const simMatrix = Array.from({ length: n }, () => new Float64Array(n));
  for (let i = 0; i < n; i++) {
    for (let j = i + 1; j < n; j++) {
      const s = simFn(i, j);
      simMatrix[i][j] = s;
      simMatrix[j][i] = s;
    }
    simMatrix[i][i] = 1;
  }

  const mergeHistory = [];

  while (clusters.length > targetClusters) {
    let bestSim = -Infinity, bestI = -1, bestJ = -1;
    for (let i = 0; i < clusters.length; i++) {
      for (let j = i + 1; j < clusters.length; j++) {
        let total = 0, count = 0;
        for (const mi of clusters[i].members) {
          for (const mj of clusters[j].members) {
            total += simMatrix[mi][mj];
            count++;
          }
        }
        const avg = total / count;
        if (avg > bestSim) { bestSim = avg; bestI = i; bestJ = j; }
      }
    }
    if (bestSim <= -Infinity) break;

    mergeHistory.push({
      clusters: [clusters[bestI].members.map((i) => TOOLS[i].id), clusters[bestJ].members.map((i) => TOOLS[i].id)],
      similarity: bestSim,
    });

    const merged = {
      id: clusters[bestI].id,
      members: [...clusters[bestI].members, ...clusters[bestJ].members],
    };
    clusters = [
      ...clusters.slice(0, bestI),
      ...clusters.slice(bestI + 1, bestJ),
      ...clusters.slice(bestJ + 1),
      merged,
    ];
  }

  return { clusters, mergeHistory };
}

// === Step 7: Derive cluster label ===
function deriveLabel(members) {
  const dimScores = {};
  for (const dim of Object.keys(SEMANTIC_DIMS)) dimScores[dim] = 0;
  for (const idx of members) {
    for (const [dim, val] of Object.entries(semVectors[idx])) {
      dimScores[dim] += val;
    }
  }
  const sorted = Object.entries(dimScores)
    .sort((a, b) => b[1] - a[1])
    .map(([dim, score]) => ({ dim, score: score / members.length }));
  return sorted;
}

// === Run ===
console.log("=== Combined Similarity: Top 20 pairs ===\n");
const pairs = [];
for (let i = 0; i < TOOLS.length; i++) {
  for (let j = i + 1; j < TOOLS.length; j++) {
    pairs.push({ i, j, sim: combinedSimilarity(i, j) });
  }
}
pairs.sort((a, b) => b.sim - a.sim);
for (const p of pairs.slice(0, 20)) {
  console.log(`  ${TOOLS[p.i].id.padEnd(28)} ↔ ${TOOLS[p.j].id.padEnd(28)} : ${p.sim.toFixed(3)}`);
}

// === Print semantic vectors for inspection ===
console.log("\n=== Tool Semantic Vectors ===\n");
for (let i = 0; i < TOOLS.length; i++) {
  const topDims = Object.entries(semVectors[i])
    .filter(([, v]) => v > 0)
    .sort((a, b) => b[1] - a[1])
    .map(([d, v]) => `${d}:${v.toFixed(2)}`)
    .join(" ");
  console.log(`  ${TOOLS[i].id.padEnd(28)} → ${topDims}`);
}

// === Cluster for sizes 4-7 ===
for (let target = 4; target <= 7; target++) {
  console.log(`\n${"=".repeat(60)}`);
  console.log(`=== CLUSTERING INTO ${target} GROUPS ===`);
  console.log(`${"=".repeat(60)}\n`);

  const { clusters, mergeHistory } = hac(TOOLS, combinedSimilarity, target);

  for (const cluster of clusters) {
    const label = deriveLabel(cluster.members);
    const topLabel = label.filter((l) => l.score > 0.05).slice(0, 3).map((l) => `${l.dim}(${l.score.toFixed(2)})`).join(" > ");

    console.log(`  ┌─ Cluster (${cluster.members.length} tools) [${topLabel}]`);
    for (const idx of cluster.members) {
      console.log(`  │  ${TOOLS[idx].id.padEnd(28)} ${TOOLS[idx].name}`);
    }
    console.log(`  └─`);
    console.log();
  }

  console.log("  Merge thresholds (last 3):");
  for (const m of mergeHistory.slice(-3)) {
    console.log(`    sim=${m.similarity.toFixed(3)}: [${m.clusters[0].join(",")}] + [${m.clusters[1].join(",")}]`);
  }
}

// === Final recommendation: find the "elbow" ===
console.log(`\n${"=".repeat(60)}`);
console.log("=== ELBOW ANALYSIS ===");
console.log(`${"=".repeat(60)}\n`);

const mergeSims = [];
for (let target = 2; target <= TOOLS.length; target++) {
  const { mergeHistory } = hac(TOOLS, combinedSimilarity, target);
  if (mergeHistory.length > 0) {
    mergeSims.push({ target, lastMergeSim: mergeHistory[mergeHistory.length - 1].similarity });
  }
}

console.log("  Target clusters → Last merge similarity:");
for (const m of mergeSims) {
  const bar = "█".repeat(Math.round(m.lastMergeSim * 50));
  console.log(`  ${m.target.toString().padStart(2)} clusters: ${m.lastMergeSim.toFixed(3)} ${bar}`);
}

// Find biggest gap (forward: where merge sim jumps most = natural cluster boundary)
let maxGap = 0, elbowK = 5;
const gaps = [];
for (let i = 1; i < mergeSims.length; i++) {
  const gap = Math.abs(mergeSims[i].lastMergeSim - mergeSims[i - 1].lastMergeSim);
  gaps.push({ from: mergeSims[i - 1].target, to: mergeSims[i].target, gap });
}
// The biggest forward jump means: going from K to K-1 required merging very similar clusters
// So K is the natural number of clusters
const biggestJump = gaps.reduce((a, b) => b.gap > a.gap ? b : a, gaps[0]);
elbowK = biggestJump.to;
console.log(`\n  Biggest similarity jump: ${biggestJump.from}→${biggestJump.to} clusters (gap=${biggestJump.gap.toFixed(3)})`);
console.log(`  → Recommended cluster count: ${elbowK}`);

// === Final output with auto-derived labels ===
console.log(`\n${"=".repeat(60)}`);
console.log(`=== FINAL: ${elbowK}-CLUSTER RESULT WITH AUTO-DERIVED LABELS ===`);
console.log(`${"=".repeat(60)}\n`);

const { clusters: finalClusters } = hac(TOOLS, combinedSimilarity, elbowK);

const LABEL_CN = {
  codec: "编解码",
  security: "安全",
  dev: "开发",
  text: "文本",
  media: "媒体",
  generate: "生成",
  network: "网络",
  math: "数学",
  time: "时间",
};

for (let ci = 0; ci < finalClusters.length; ci++) {
  const cluster = finalClusters[ci];
  const label = deriveLabel(cluster.members);
  const primaryDim = label[0].dim;
  const cnLabel = LABEL_CN[primaryDim] || primaryDim;
  const allDims = label.filter((l) => l.score > 0.05).slice(0, 3).map((l) => `${LABEL_CN[l.dim] || l.dim}(${l.score.toFixed(2)})`).join(" > ");

  console.log(`  Category: "${primaryDim}" (中文: "${cnLabel}")`);
  console.log(`  Dimension scores: ${allDims}`);
  console.log(`  Tools (${cluster.members.length}):`);
  for (const idx of cluster.members) {
    console.log(`    - ${TOOLS[idx].id}`);
  }
  console.log();
}

// === Output as TypeScript-compatible format ===
console.log(`\n${"=".repeat(60)}`);
console.log("=== TYPESCRIPT CATEGORY DEFINITION ===");
console.log(`${"=".repeat(60)}\n`);

const catDefs = [];
const toolAssignments = {};
for (let ci = 0; ci < finalClusters.length; ci++) {
  const cluster = finalClusters[ci];
  const label = deriveLabel(cluster.members);
  const primaryDim = label[0].dim;
  const cnLabel = LABEL_CN[primaryDim] || primaryDim;
  catDefs.push({ key: primaryDim, label: cnLabel, order: ci });
  for (const idx of cluster.members) {
    toolAssignments[TOOLS[idx].id] = primaryDim;
  }
}

console.log("// Auto-derived categories from HAC clustering:");
console.log("export type ToolCategory =");
for (const cat of catDefs) {
  console.log(`  | "${cat.key}"`);
}
console.log(";");
console.log();
console.log("export const TOOL_CATEGORIES: ToolCategoryMeta[] = [");
for (const cat of catDefs) {
  console.log(`  { key: "${cat.key}", label: "${cat.label}", order: ${cat.order} },`);
}
console.log("];");
console.log();
console.log("// Tool category assignments:");
for (const [toolId, cat] of Object.entries(toolAssignments)) {
  console.log(`// ${toolId} → "${cat}"`);
}