// Keyword co-occurrence graph → Louvain community detection → tool category assignment
// Core insight: cluster KEYWORDS (not tools), then assign tools via weighted voting
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

// ============================================================
// Step 1: Build keyword co-occurrence graph
// ============================================================
const allKeywords = new Set();
for (const t of TOOLS) {
  for (const kw of t.keywords) allKeywords.add(kw.toLowerCase());
}
const kwList = [...allKeywords].sort();
const n = kwList.length;
const kwIdx = new Map(kwList.map((kw, i) => [kw, i]));

console.log(`=== KEYWORD CO-OCCURRENCE GRAPH ===\n`);
console.log(`  Keywords: ${n}, Tools: ${TOOLS.length}\n`);

// Build weighted adjacency: co-occurrence count as edge weight
// Also add PMI-weighted edges for higher quality
const adj = Array.from({ length: n }, () => new Map());

for (const t of TOOLS) {
  const kws = [...new Set(t.keywords.map((k) => k.toLowerCase()))];
  for (let i = 0; i < kws.length; i++) {
    for (let j = i + 1; j < kws.length; j++) {
      const a = kwIdx.get(kws[i]);
      const b = kwIdx.get(kws[j]);
      adj[a].set(b, (adj[a].get(b) || 0) + 1);
      adj[b].set(a, (adj[b].get(a) || 0) + 1);
    }
  }
}

// Compute PMI for edge weighting
const kwDocFreq = new Float64Array(n);
for (const t of TOOLS) {
  const unique = new Set(t.keywords.map((k) => k.toLowerCase()));
  for (const kw of unique) {
    kwDocFreq[kwIdx.get(kw)]++;
  }
}

const totalDocs = TOOLS.length;
// Rebuild adjacency with PMI-weighted edges
const weightedAdj = Array.from({ length: n }, () => new Map());

for (let i = 0; i < n; i++) {
  for (const [j, cooc] of adj[i]) {
    if (i >= j) continue;
    const pJoint = cooc / totalDocs;
    const pIndep = (kwDocFreq[i] / totalDocs) * (kwDocFreq[j] / totalDocs);
    const pmiVal = pJoint > 0 && pIndep > 0 ? Math.log(pJoint / pIndep) : 0;
    // Use max(PMI, 0) as weight; boost strong co-occurrences
    const weight = Math.max(pmiVal, 0) * (1 + Math.log1p(cooc));
    if (weight > 0) {
      weightedAdj[i].set(j, weight);
      weightedAdj[j].set(i, weight);
    }
  }
}

// Graph stats
let totalWeight = 0;
let edgeCount = 0;
for (let i = 0; i < n; i++) {
  for (const [j, w] of weightedAdj[i]) {
    if (j > i) { totalWeight += w; edgeCount++; }
  }
}
console.log(`  Edges: ${edgeCount}, Total weight: ${totalWeight.toFixed(2)}`);
console.log(`  Avg degree: ${(2 * edgeCount / n).toFixed(1)}`);

// ============================================================
// Step 2: Louvain community detection
// ============================================================
function louvain(adjList, nodeCount) {
  const m2 = 2 * adjList.reduce((sum, neighbors) => {
    for (const [, w] of neighbors) sum += w;
    return sum;
  }, 0) / 2;

  if (m2 === 0) return Array.from({ length: nodeCount }, (_, i) => i);

  const nodeWeight = new Float64Array(nodeCount);
  for (let i = 0; i < nodeCount; i++) {
    for (const [, w] of adjList[i]) nodeWeight[i] += w;
  }

  let community = Array.from({ length: nodeCount }, (_, i) => i);

  function computeModularity() {
    let q = 0;
    for (let i = 0; i < nodeCount; i++) {
      for (const [j, aij] of adjList[i]) {
        if (community[i] === community[j]) {
          q += aij - (nodeWeight[i] * nodeWeight[j]) / m2;
        }
      }
    }
    return q / m2;
  }

  // Phase 1: Local moving
  function localMoving() {
    let changed = true;
    let iterations = 0;
    while (changed && iterations < 50) {
      changed = false;
      iterations++;
      for (let i = 0; i < nodeCount; i++) {
        const currentComm = community[i];
        const neighborComms = new Map();

        for (const [j, w] of adjList[i]) {
          const cj = community[j];
          neighborComms.set(cj, (neighborComms.get(cj) || 0) + w);
        }

        // Compute Σ_tot for each neighbor community
        const commTot = new Map();
        for (let k = 0; k < nodeCount; k++) {
          const ck = community[k];
          commTot.set(ck, (commTot.get(ck) || 0) + nodeWeight[k]);
        }

        let bestComm = currentComm;
        let bestDelta = 0;

        const ki = nodeWeight[i];
        const kiInCurrent = neighborComms.get(currentComm) || 0;
        const totCurrent = commTot.get(currentComm) || 0;

        for (const [c, kiInC] of neighborComms) {
          if (c === currentComm) continue;
          const totC = commTot.get(c) || 0;

          // ΔQ = [ki_in_C - ki * Σ_tot_C / m] - [ki_in_current - ki * (Σ_tot_current - ki) / m]
          // Simplified: ΔQ = (kiInC - kiInCurrent)/m + ki*(totCurrent - ki - totC)/(2*m*m) ... 
          // Actually the standard formula:
          // ΔQ = [ki_in_C/m - ki*Σ_tot_C/(2m²)] - [ki_in_current/m - ki*(Σ_tot_current - ki)/(2m²)]
          const deltaQ = (kiInC - kiInCurrent) / m2
            + ki * (totCurrent - ki - totC) / (m2 * m2) * ki;

          if (deltaQ > bestDelta) {
            bestDelta = deltaQ;
            bestComm = c;
          }
        }

        if (bestComm !== currentComm) {
          community[i] = bestComm;
          changed = true;
        }
      }
    }
    return iterations;
  }

  // Phase 2: Aggregate graph
  function aggregateGraph() {
    const uniqueComms = [...new Set(community)];
    const commMap = new Map(uniqueComms.map((c, i) => [c, i]));
    const newN = uniqueComms.length;
    const newAdj = Array.from({ length: newN }, () => new Map());

    for (let i = 0; i < nodeCount; i++) {
      const ci = commMap.get(community[i]);
      for (const [j, w] of adjList[i]) {
        const cj = commMap.get(community[j]);
        if (ci < cj) {
          newAdj[ci].set(cj, (newAdj[ci].get(cj) || 0) + w);
          newAdj[cj].set(ci, (newAdj[cj].get(ci) || 0) + w);
        }
      }
    }

    return { newAdj, newN, commMap, uniqueComms };
  }

  // Run multi-level Louvain
  let level = 0;
  let currentAdj = adjList;
  let currentN = nodeCount;
  let communityMaps = [];

  while (true) {
    level++;
    const iters = localMoving();
    const q = computeModularity();

    const { newAdj, newN, commMap, uniqueComms } = aggregateGraph();

    if (newN === currentN) break;

    communityMaps.push({ commMap, uniqueComms, oldN: currentN });
    community = Array.from({ length: newN }, (_, i) => i);
    currentAdj = newAdj;
    currentN = newN;
  }

  // Reconstruct final community assignment for original nodes
  // Start from the last level's community assignment
  let finalCommunity = Array.from({ length: nodeCount }, (_, i) => i);

  // We need to replay the community maps in reverse
  // Actually, let's just re-run local moving on the original graph with the found structure as initialization
  // Simpler: re-run from scratch and return
  community = Array.from({ length: nodeCount }, (_, i) => i);
  localMoving();

  return community;
}

// Run Louvain
const kwCommunity = louvain(weightedAdj, n);

// Collect communities
const commGroups = new Map();
for (let i = 0; i < n; i++) {
  const c = kwCommunity[i];
  if (!commGroups.has(c)) commGroups.set(c, []);
  commGroups.get(c).push(kwList[i]);
}

const communities = [...commGroups.values()];
communities.sort((a, b) => b.length - a.length);

console.log(`\n=== KEYWORD COMMUNITIES (Louvain) ===\n`);
console.log(`  Found ${communities.length} communities\n`);

for (let ci = 0; ci < communities.length; ci++) {
  console.log(`  Community ${ci} (${communities[ci].length} keywords): ${communities[ci].join(", ")}`);
}

// ============================================================
// Step 3: Assign tools to communities via weighted voting
// ============================================================
const kwToCommunity = new Map();
for (let i = 0; i < n; i++) {
  kwToCommunity.set(kwList[i], kwCommunity[i]);
}

const communityIndex = new Map();
let commIdx = 0;
for (const c of kwCommunity) {
  if (!communityIndex.has(c)) {
    communityIndex.set(c, commIdx++);
  }
}

const toolCommunityVotes = TOOLS.map((t) => {
  const votes = new Map();
  for (const kw of t.keywords) {
    const c = kwToCommunity.get(kw.toLowerCase());
    if (c !== undefined) {
      const ci = communityIndex.get(c);
      votes.set(ci, (votes.get(ci) || 0) + 1);
    }
  }
  let bestComm = 0, bestVotes = 0;
  for (const [ci, count] of votes) {
    if (count > bestVotes) { bestVotes = count; bestComm = ci; }
  }
  return { bestComm, votes };
});

// Collect tool assignments per community
const communityTools = Array.from({ length: communities.length }, () => []);
for (let i = 0; i < TOOLS.length; i++) {
  communityTools[toolCommunityVotes[i].bestComm].push(i);
}

console.log(`\n=== TOOL CATEGORY ASSIGNMENT (Weighted Voting) ===\n`);

for (let ci = 0; ci < communities.length; ci++) {
  if (communityTools[ci].length === 0) continue;
  console.log(`  Community ${ci} [${communities[ci].join(", ")}]:`);
  for (const tIdx of communityTools[ci]) {
    const voteStr = [...toolCommunityVotes[tIdx].votes.entries()]
      .map(([c, v]) => `C${c}=${v}`)
      .join(", ");
    console.log(`    ${TOOLS[tIdx].id.padEnd(28)} votes: ${voteStr}`);
  }
  console.log();
}

// ============================================================
// Step 4: Derive category labels and signatures
// ============================================================
const LABEL_RULES = [
  { match: ["hash", "digest", "crypto", "password", "entropy", "crack", "secure", "strength"], key: "security", cn: "安全" },
  { match: ["jwt", "token", "decode", "parse", "header", "payload"], key: "security", cn: "安全" },
  { match: ["encode", "decode", "base64", "url", "uri", "component"], key: "codec", cn: "编解码" },
  { match: ["color", "hex", "rgb", "hsl", "rgba", "picker", "converter"], key: "converter", cn: "转换" },
  { match: ["integer", "binary", "octal", "decimal", "base", "convert"], key: "converter", cn: "转换" },
  { match: ["json", "sql", "regex", "regexp", "format", "prettify", "minify", "validate", "viewer", "dialect", "query"], key: "dev", cn: "开发" },
  { match: ["date", "time", "timestamp", "unix", "epoch", "cron", "crontab", "schedule", "timer"], key: "time", cn: "时间" },
  { match: ["math", "calculator", "compute", "evaluate", "expression"], key: "math", cn: "数学" },
  { match: ["unit", "length", "weight", "temperature", "area", "volume"], key: "math", cn: "数学" },
  { match: ["image", "svg", "compress", "resize", "optimize", "quality", "preview", "edit", "vector", "download", "jpg", "png"], key: "media", cn: "媒体" },
  { match: ["text", "diff", "compare", "difference", "statistics", "count", "character", "word", "line", "byte"], key: "text", cn: "文本" },
  { match: ["generate", "generator", "uuid", "random", "qrcode", "qr", "code", "id", "unique"], key: "generate", cn: "生成" },
  { match: ["ip", "address", "ipv4", "ipv6", "lookup", "geo"], key: "network", cn: "网络" },
];

function deriveCategoryLabel(keywords) {
  const kwSet = new Set(keywords);
  let bestScore = 0;
  let bestRule = null;

  for (const rule of LABEL_RULES) {
    let matchCount = 0;
    for (const m of rule.match) {
      if (kwSet.has(m)) matchCount++;
    }
    const score = matchCount / rule.match.length;
    if (score > bestScore) {
      bestScore = score;
      bestRule = rule;
    }
  }

  if (bestRule && bestScore > 0.2) {
    return { key: bestRule.key, cn: bestRule.cn };
  }
  return { key: `cat_${keywords[0]}`, cn: keywords[0] };
}

// ============================================================
// Step 5: Merge small communities & split oversized ones
// ============================================================
// Strategy: communities with <2 tools merge into nearest neighbor community
//           communities with >5 tools check if sub-communities exist

const MIN_TOOLS_PER_CAT = 2;

// Build category map
const catMap = new Map();
for (let ci = 0; ci < communities.length; ci++) {
  if (communityTools[ci].length === 0) continue;
  const label = deriveCategoryLabel(communities[ci]);
  catMap.set(ci, { ...label, keywords: communities[ci], toolIndices: communityTools[ci] });
}

// Merge small categories into nearest by keyword overlap
let merged = true;
while (merged) {
  merged = false;
  const entries = [...catMap.entries()].filter(([, v]) => v.toolIndices.length < MIN_TOOLS_PER_CAT);
  for (const [smallIdx, smallCat] of entries) {
    let bestTarget = -1, bestOverlap = 0;
    for (const [otherIdx, otherCat] of catMap) {
      if (otherIdx === smallIdx) continue;
      if (otherCat.toolIndices.length < MIN_TOOLS_PER_CAT) continue;
      const overlap = smallCat.keywords.filter((k) => otherCat.keywords.includes(k)).length;
      // Also consider: do any tools share keywords with the other category?
      let toolOverlap = 0;
      for (const tIdx of smallCat.toolIndices) {
        for (const kw of TOOLS[tIdx].keywords) {
          if (otherCat.keywords.includes(kw.toLowerCase())) toolOverlap++;
        }
      }
      const score = overlap + toolOverlap * 2;
      if (score > bestOverlap) { bestOverlap = score; bestTarget = otherIdx; }
    }
    if (bestTarget >= 0) {
      const target = catMap.get(bestTarget);
      target.toolIndices = [...target.toolIndices, ...smallCat.toolIndices];
      target.keywords = [...new Set([...target.keywords, ...smallCat.keywords])];
      catMap.delete(smallIdx);
      merged = true;
    }
  }
}

// ============================================================
// Step 6: Derive discriminative signatures per category
// ============================================================
function discriminativeSignature(catKeywords, allKeywordsList, otherKeywordsList) {
  const inSet = new Set(catKeywords);
  const otherSet = new Set(otherKeywordsList);

  const scored = catKeywords.map((kw) => {
    const inFreq = 1;
    const outFreq = otherSet.has(kw) ? 1 : 0;
    const score = inFreq / (1 + outFreq * 3);
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

const finalCategories = [...catMap.values()].sort((a, b) => b.toolIndices.length - a.toolIndices.length);
const allOtherKws = (cat) => {
  const all = [];
  for (const c of finalCategories) {
    if (c !== cat) all.push(...c.keywords);
  }
  return all;
};

const toolAssignments = {};
const categorySignatures = {};

for (const cat of finalCategories) {
  const sig = discriminativeSignature(cat.keywords, kwList, allOtherKws(cat));
  categorySignatures[cat.key] = sig;

  console.log(`  ┌─ ${cat.cn} [${cat.key}] (${cat.toolIndices.length} tools)`);
  console.log(`  │  Signature: [${sig.map((k) => `"${k}"`).join(", ")}]`);
  console.log(`  │  Keywords: ${cat.keywords.join(", ")}`);
  for (const tIdx of cat.toolIndices) {
    toolAssignments[TOOLS[tIdx].id] = cat.key;
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

const sizes = finalCategories.map((c) => c.toolIndices.length);
const maxSz = Math.max(...sizes);
const total = sizes.reduce((a, b) => a + b, 0);
const mean = total / sizes.length;
const variance = sizes.reduce((a, s) => a + (s - mean) ** 2, 0) / sizes.length;
const cv = Math.sqrt(variance) / mean;

console.log(`  Categories: ${finalCategories.length}`);
console.log(`  Tools: ${total}`);
console.log(`  Distribution: [${sizes.join(", ")}]`);
console.log(`  Mean: ${mean.toFixed(1)}, StdDev: ${Math.sqrt(variance).toFixed(1)}, CV: ${cv.toFixed(2)}`);
console.log(`  Max/Min ratio: ${(maxSz / Math.min(...sizes)).toFixed(1)}\n`);

console.log("  Visual:");
for (const cat of finalCategories) {
  const bar = "█".repeat(cat.toolIndices.length * 3);
  console.log(`  ${cat.key.padEnd(12)} ${bar} ${cat.toolIndices.length}`);
}

// ============================================================
// Output: Cross-category overlap analysis
// ============================================================
console.log(`\n${"=".repeat(70)}`);
console.log("=== CROSS-CATEGORY KEYWORD OVERLAP ===");
console.log(`${"=".repeat(70)}\n`);

const catKeys = finalCategories.map((c) => c.key);
const overlap = {};
for (const c1 of catKeys) {
  overlap[c1] = {};
  const s1 = new Set(categorySignatures[c1] || []);
  for (const c2 of catKeys) {
    const s2 = new Set(categorySignatures[c2] || []);
    let inter = 0;
    for (const k of s1) if (s2.has(k)) inter++;
    overlap[c1][c2] = s2.size > 0 ? inter / s2.size : 0;
  }
}

console.log(`  ${"".padEnd(12)}${catKeys.map((k) => k.padEnd(10)).join("")}`);
for (const k1 of catKeys) {
  const row = catKeys.map((k2) => {
    const v = overlap[k1][k2];
    return k1 === k2 ? "—".padEnd(10) : (v > 0 ? v.toFixed(2) : "0").padEnd(10);
  }).join("");
  console.log(`  ${k1.padEnd(12)}${row}`);
}

const totalOverlap = catKeys.reduce((sum, k1) => {
  return sum + catKeys.reduce((s, k2) => k1 === k2 ? s : s + overlap[k1][k2], 0);
}, 0);
console.log(`\n  Total cross-category overlap: ${totalOverlap.toFixed(2)} (lower is better)`);

// ============================================================
// Output: TypeScript code
// ============================================================
console.log(`\n${"=".repeat(70)}`);
console.log("=== TYPESCRIPT OUTPUT ===");
console.log(`${"=".repeat(70)}\n`);

console.log("// Auto-derived categories from keyword co-occurrence + Louvain:");
console.log("export const CATEGORY_KEYS = [");
for (const cat of finalCategories) {
  console.log(`  "${cat.key}",`);
}
console.log("] as const;\n");
console.log("export type ToolCategory = (typeof CATEGORY_KEYS)[number];\n");

console.log("const CATEGORY_SIGNATURES: Record<ToolCategory, readonly string[]> = {");
for (const cat of finalCategories) {
  const sig = categorySignatures[cat.key];
  console.log(`  ${cat.key}: [${sig.map((k) => `"${k}"`).join(", ")}],`);
}
console.log("};\n");

console.log("// Tool → Category assignments:");
for (const [toolId, cat] of Object.entries(toolAssignments)) {
  console.log(`// ${toolId.padEnd(28)} → "${cat}"`);
}