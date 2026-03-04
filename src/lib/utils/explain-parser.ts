// ── EXPLAIN ANALYZE parser ──
// Handles both TEXT format (default) and JSON format (EXPLAIN FORMAT JSON).
// No external dependencies.

// ── Types ──

export interface ExplainNode {
  nodeType: string;
  relation?: string;
  schema?: string;
  alias?: string;
  startupCost: number;
  totalCost: number;
  planRows: number;
  planWidth: number;
  actualStartup?: number;
  actualTotal?: number;
  actualRows?: number;
  actualLoops?: number;
  exclusiveTimeMs?: number;
  percentOfTotal?: number;
  filter?: string;
  joinFilter?: string;
  indexCond?: string;
  sortKey?: string[];
  hashCond?: string;
  output?: string[];
  buffers?: {
    sharedHit?: number;
    sharedRead?: number;
    sharedWritten?: number;
  };
  children: ExplainNode[];
  depth: number;
  rowEstimateFactor?: number;
}

export interface ExplainPlan {
  nodes: ExplainNode;
  planningTimeMs?: number;
  executionTimeMs?: number;
  totalTimeMs?: number;
  isAnalyze: boolean;
  triggers?: { name: string; time: number; calls: number }[];
}

// ── TEXT format parser ──

// Regex for the main node line:
//   NodeType [on relation alias] [using index] (cost=X..Y rows=N width=W) [(actual time=X..Y rows=N loops=L)]
const NODE_LINE_RE =
  /^(.*?)\s*\(cost=(\d+(?:\.\d+)?)\.\.(\d+(?:\.\d+)?)\s+rows=(\d+)\s+width=(\d+)\)(?:\s+\(actual time=(\d+(?:\.\d+)?)\.\.(\d+(?:\.\d+)?)\s+rows=(\d+)\s+loops=(\d+)\))?/;

const PLANNING_TIME_RE = /^Planning Time:\s+(\d+(?:\.\d+)?)\s+ms$/;
const EXECUTION_TIME_RE = /^Execution Time:\s+(\d+(?:\.\d+)?)\s+ms$/;
const TRIGGER_RE =
  /^Trigger\s+(.+?):\s+time=(\d+(?:\.\d+)?)\s+calls=(\d+)/;

// Attribute lines: keyword followed by colon
const FILTER_RE = /^\s*Filter:\s+(.+)$/;
const JOIN_FILTER_RE = /^\s*Join Filter:\s+(.+)$/;
const INDEX_COND_RE = /^\s*Index Cond:\s+(.+)$/;
const HASH_COND_RE = /^\s*Hash Cond:\s+(.+)$/;
const SORT_KEY_RE = /^\s*Sort Key:\s+(.+)$/;
const OUTPUT_RE = /^\s*Output:\s+(.+)$/;
const BUFFERS_RE =
  /^\s*Buffers:\s+(.+)$/;

/**
 * Determine the indentation depth of a text plan line.
 * We strip the leading "->  " arrows and count the total leading whitespace
 * plus arrows to compute a depth integer.
 */
function lineDepth(line: string): number {
  // Count leading spaces before any content or arrow
  let pos = 0;
  while (pos < line.length && line[pos] === ' ') {
    pos++;
  }
  // Check for arrow
  if (line.substring(pos, pos + 2) === '->') {
    // Arrow adds to depth — count spaces after arrow
    pos += 2;
    while (pos < line.length && line[pos] === ' ') {
      pos++;
    }
  }
  // Every 6 chars of leading content roughly equals one depth level,
  // but we actually track depths by indentation count and normalize later.
  return pos;
}

/**
 * Strip leading whitespace and `->` arrow from a plan line, returning the content.
 */
function stripIndent(line: string): string {
  return line.replace(/^\s*(?:->\s*)?/, '');
}

/**
 * Parse the node type and optional relation/alias/schema from the text before `(cost=...)`.
 *
 * Examples:
 *   "Seq Scan on users u"       -> nodeType="Seq Scan", relation="users", alias="u"
 *   "Index Scan using idx_id on public.users u" -> nodeType="Index Scan", relation="users", schema="public", alias="u"
 *   "Hash Join"                  -> nodeType="Hash Join"
 *   "Sort"                       -> nodeType="Sort"
 */
function parseNodeHeader(raw: string): {
  nodeType: string;
  relation?: string;
  schema?: string;
  alias?: string;
} {
  const trimmed = raw.trim();

  // Try "... using <index> on <schema.relation> <alias>"
  const usingOnRe =
    /^(.+?)\s+using\s+\S+\s+on\s+(?:(\w+)\.)?(\w+)(?:\s+(\w+))?$/;
  let m = usingOnRe.exec(trimmed);
  if (m) {
    return {
      nodeType: m[1].trim(),
      schema: m[2] || undefined,
      relation: m[3],
      alias: m[4] || undefined,
    };
  }

  // Try "... on <schema.relation> <alias>"
  const onRe = /^(.+?)\s+on\s+(?:(\w+)\.)?(\w+)(?:\s+(\w+))?$/;
  m = onRe.exec(trimmed);
  if (m) {
    return {
      nodeType: m[1].trim(),
      schema: m[2] || undefined,
      relation: m[3],
      alias: m[4] || undefined,
    };
  }

  // Try "... using <index>"
  const usingRe = /^(.+?)\s+using\s+(\S+)$/;
  m = usingRe.exec(trimmed);
  if (m) {
    return {
      nodeType: m[1].trim(),
      relation: m[2],
    };
  }

  return { nodeType: trimmed };
}

function parseBuffersLine(text: string): ExplainNode['buffers'] {
  const buffers: ExplainNode['buffers'] = {};
  const hitMatch = /shared hit=(\d+)/.exec(text);
  if (hitMatch) buffers.sharedHit = parseInt(hitMatch[1], 10);
  const readMatch = /shared read=(\d+)/.exec(text);
  if (readMatch) buffers.sharedRead = parseInt(readMatch[1], 10);
  const writtenMatch = /shared written=(\d+)/.exec(text);
  if (writtenMatch) buffers.sharedWritten = parseInt(writtenMatch[1], 10);
  return buffers;
}

export function parseExplainText(lines: string[]): ExplainPlan | null {
  if (lines.length === 0) return null;

  // Collect nodes and summary info
  let planningTimeMs: number | undefined;
  let executionTimeMs: number | undefined;
  const triggers: { name: string; time: number; calls: number }[] = [];

  // First pass: identify node lines vs attribute lines vs summary lines
  interface RawNode {
    indent: number;
    node: ExplainNode;
    lineIndex: number;
  }

  const rawNodes: RawNode[] = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!line.trim()) continue;

    // Check summary lines
    const planMatch = PLANNING_TIME_RE.exec(line.trim());
    if (planMatch) {
      planningTimeMs = parseFloat(planMatch[1]);
      continue;
    }

    const execMatch = EXECUTION_TIME_RE.exec(line.trim());
    if (execMatch) {
      executionTimeMs = parseFloat(execMatch[1]);
      continue;
    }

    const trigMatch = TRIGGER_RE.exec(line.trim());
    if (trigMatch) {
      triggers.push({
        name: trigMatch[1],
        time: parseFloat(trigMatch[2]),
        calls: parseInt(trigMatch[3], 10),
      });
      continue;
    }

    // Try to parse as a node line
    const stripped = stripIndent(line);
    const nodeMatch = NODE_LINE_RE.exec(stripped);
    if (nodeMatch) {
      const header = parseNodeHeader(nodeMatch[1]);
      const indent = lineDepth(line);

      const node: ExplainNode = {
        nodeType: header.nodeType,
        relation: header.relation,
        schema: header.schema,
        alias: header.alias,
        startupCost: parseFloat(nodeMatch[2]),
        totalCost: parseFloat(nodeMatch[3]),
        planRows: parseInt(nodeMatch[4], 10),
        planWidth: parseInt(nodeMatch[5], 10),
        children: [],
        depth: 0, // computed later
      };

      if (nodeMatch[6] !== undefined) {
        node.actualStartup = parseFloat(nodeMatch[6]);
        node.actualTotal = parseFloat(nodeMatch[7]);
        node.actualRows = parseInt(nodeMatch[8], 10);
        node.actualLoops = parseInt(nodeMatch[9], 10);
      }

      rawNodes.push({ indent, node, lineIndex: i });
      continue;
    }

    // Attribute line — attach to the most recent node
    if (rawNodes.length > 0) {
      const parent = rawNodes[rawNodes.length - 1].node;

      let attrMatch = FILTER_RE.exec(line);
      if (attrMatch) {
        parent.filter = attrMatch[1].trim();
        continue;
      }

      attrMatch = JOIN_FILTER_RE.exec(line);
      if (attrMatch) {
        parent.joinFilter = attrMatch[1].trim();
        continue;
      }

      attrMatch = INDEX_COND_RE.exec(line);
      if (attrMatch) {
        parent.indexCond = attrMatch[1].trim();
        continue;
      }

      attrMatch = HASH_COND_RE.exec(line);
      if (attrMatch) {
        parent.hashCond = attrMatch[1].trim();
        continue;
      }

      attrMatch = SORT_KEY_RE.exec(line);
      if (attrMatch) {
        parent.sortKey = attrMatch[1].split(',').map((s) => s.trim());
        continue;
      }

      attrMatch = OUTPUT_RE.exec(line);
      if (attrMatch) {
        parent.output = attrMatch[1].split(',').map((s) => s.trim());
        continue;
      }

      attrMatch = BUFFERS_RE.exec(line);
      if (attrMatch) {
        parent.buffers = parseBuffersLine(attrMatch[1]);
        continue;
      }
    }
  }

  if (rawNodes.length === 0) return null;

  // Build the tree from indentation levels.
  // Assign depth based on unique sorted indentation values.
  const uniqueIndents = [...new Set(rawNodes.map((r) => r.indent))].sort(
    (a, b) => a - b,
  );
  const indentToDepth = new Map<number, number>();
  uniqueIndents.forEach((indent, idx) => indentToDepth.set(indent, idx));

  for (const raw of rawNodes) {
    raw.node.depth = indentToDepth.get(raw.indent) ?? 0;
  }

  // Build tree using a stack approach
  const root = rawNodes[0].node;
  const stack: ExplainNode[] = [root];

  for (let i = 1; i < rawNodes.length; i++) {
    const current = rawNodes[i].node;

    // Pop stack until we find a node whose depth is less than current
    while (stack.length > 1 && stack[stack.length - 1].depth >= current.depth) {
      stack.pop();
    }

    const parentNode = stack[stack.length - 1];
    parentNode.children.push(current);
    stack.push(current);
  }

  const isAnalyze = root.actualTotal !== undefined;

  const plan: ExplainPlan = {
    nodes: root,
    planningTimeMs,
    executionTimeMs,
    isAnalyze,
  };

  if (planningTimeMs !== undefined && executionTimeMs !== undefined) {
    plan.totalTimeMs = planningTimeMs + executionTimeMs;
  }

  if (triggers.length > 0) {
    plan.triggers = triggers;
  }

  if (isAnalyze) {
    computePostProcessing(plan);
  }

  return plan;
}

// ── JSON format parser ──

interface PgJsonPlan {
  'Node Type': string;
  'Relation Name'?: string;
  'Schema'?: string;
  'Alias'?: string;
  'Startup Cost': number;
  'Total Cost': number;
  'Plan Rows': number;
  'Plan Width': number;
  'Actual Startup Time'?: number;
  'Actual Total Time'?: number;
  'Actual Rows'?: number;
  'Actual Loops'?: number;
  'Filter'?: string;
  'Join Filter'?: string;
  'Index Cond'?: string;
  'Sort Key'?: string[];
  'Hash Cond'?: string;
  'Output'?: string[];
  'Shared Hit Blocks'?: number;
  'Shared Read Blocks'?: number;
  'Shared Written Blocks'?: number;
  Plans?: PgJsonPlan[];
  [key: string]: unknown;
}

interface PgJsonRoot {
  Plan: PgJsonPlan;
  'Planning Time'?: number;
  'Execution Time'?: number;
  Triggers?: { 'Trigger Name': string; Time: number; Calls: number }[];
}

function convertJsonNode(plan: PgJsonPlan, depth: number): ExplainNode {
  const node: ExplainNode = {
    nodeType: plan['Node Type'],
    relation: plan['Relation Name'],
    schema: plan['Schema'],
    alias: plan['Alias'],
    startupCost: plan['Startup Cost'],
    totalCost: plan['Total Cost'],
    planRows: plan['Plan Rows'],
    planWidth: plan['Plan Width'],
    children: [],
    depth,
  };

  if (plan['Actual Startup Time'] !== undefined) {
    node.actualStartup = plan['Actual Startup Time'];
  }
  if (plan['Actual Total Time'] !== undefined) {
    node.actualTotal = plan['Actual Total Time'];
  }
  if (plan['Actual Rows'] !== undefined) {
    node.actualRows = plan['Actual Rows'];
  }
  if (plan['Actual Loops'] !== undefined) {
    node.actualLoops = plan['Actual Loops'];
  }

  if (plan['Filter']) node.filter = plan['Filter'];
  if (plan['Join Filter']) node.joinFilter = plan['Join Filter'];
  if (plan['Index Cond']) node.indexCond = plan['Index Cond'];
  if (plan['Sort Key']) node.sortKey = plan['Sort Key'];
  if (plan['Hash Cond']) node.hashCond = plan['Hash Cond'];
  if (plan['Output']) node.output = plan['Output'];

  const hasBuffers =
    plan['Shared Hit Blocks'] !== undefined ||
    plan['Shared Read Blocks'] !== undefined ||
    plan['Shared Written Blocks'] !== undefined;
  if (hasBuffers) {
    node.buffers = {};
    if (plan['Shared Hit Blocks'] !== undefined)
      node.buffers.sharedHit = plan['Shared Hit Blocks'];
    if (plan['Shared Read Blocks'] !== undefined)
      node.buffers.sharedRead = plan['Shared Read Blocks'];
    if (plan['Shared Written Blocks'] !== undefined)
      node.buffers.sharedWritten = plan['Shared Written Blocks'];
  }

  if (plan.Plans) {
    for (const child of plan.Plans) {
      node.children.push(convertJsonNode(child, depth + 1));
    }
  }

  return node;
}

export function parseExplainJson(json: string): ExplainPlan | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(json);
  } catch {
    return null;
  }

  // PostgreSQL EXPLAIN (FORMAT JSON) returns an array with one element
  let root: PgJsonRoot;
  if (Array.isArray(parsed) && parsed.length > 0 && parsed[0]?.Plan) {
    root = parsed[0] as PgJsonRoot;
  } else if (
    typeof parsed === 'object' &&
    parsed !== null &&
    'Plan' in parsed
  ) {
    root = parsed as PgJsonRoot;
  } else {
    return null;
  }

  const nodes = convertJsonNode(root.Plan, 0);
  const isAnalyze = nodes.actualTotal !== undefined;

  const plan: ExplainPlan = {
    nodes,
    isAnalyze,
  };

  if (root['Planning Time'] !== undefined) {
    plan.planningTimeMs = root['Planning Time'];
  }
  if (root['Execution Time'] !== undefined) {
    plan.executionTimeMs = root['Execution Time'];
  }
  if (
    plan.planningTimeMs !== undefined &&
    plan.executionTimeMs !== undefined
  ) {
    plan.totalTimeMs = plan.planningTimeMs + plan.executionTimeMs;
  }

  if (root.Triggers && root.Triggers.length > 0) {
    plan.triggers = root.Triggers.map((t) => ({
      name: t['Trigger Name'],
      time: t.Time,
      calls: t.Calls,
    }));
  }

  if (isAnalyze) {
    computePostProcessing(plan);
  }

  return plan;
}

// ── Post-processing ──

/**
 * Compute exclusive time, percent of total, and row estimate factor
 * for every node in the tree.
 */
function computePostProcessing(plan: ExplainPlan): void {
  const root = plan.nodes;
  if (root.actualTotal === undefined) return;

  const rootTotalTimeMs = (root.actualTotal ?? 0) * (root.actualLoops ?? 1);

  function walk(node: ExplainNode): void {
    const nodeTimeMs = (node.actualTotal ?? 0) * (node.actualLoops ?? 1);

    // Sum children's inclusive time
    let childrenTimeMs = 0;
    for (const child of node.children) {
      childrenTimeMs += (child.actualTotal ?? 0) * (child.actualLoops ?? 1);
    }

    node.exclusiveTimeMs = Math.max(0, nodeTimeMs - childrenTimeMs);

    if (rootTotalTimeMs > 0) {
      node.percentOfTotal = (node.exclusiveTimeMs / rootTotalTimeMs) * 100;
    } else {
      node.percentOfTotal = 0;
    }

    // Row estimate factor: actualRows / planRows
    if (node.actualRows !== undefined && node.planRows > 0) {
      node.rowEstimateFactor = node.actualRows / node.planRows;
    } else if (node.actualRows !== undefined && node.planRows === 0) {
      // planRows was 0 but actual rows exist — use actualRows as the factor
      node.rowEstimateFactor = node.actualRows > 0 ? node.actualRows : 1;
    }

    for (const child of node.children) {
      walk(child);
    }
  }

  walk(root);
}

// ── Auto-detect and unified parser ──

export function parseExplain(input: string | string[]): ExplainPlan | null {
  let text: string;
  if (Array.isArray(input)) {
    text = input.join('\n');
  } else {
    text = input;
  }

  const trimmed = text.trimStart();

  // JSON format: starts with `[` or `{`
  if (trimmed.startsWith('[') || trimmed.startsWith('{')) {
    return parseExplainJson(trimmed);
  }

  // TEXT format: split into lines
  const lines = Array.isArray(input)
    ? input
    : text.split('\n');
  return parseExplainText(lines);
}

// ── Detection utility ──

/**
 * Detect whether a query result looks like EXPLAIN output.
 * Checks: single column named "QUERY PLAN", and the first cell matches
 * EXPLAIN patterns (contains cost= or has a known node type prefix).
 */
/** Check if a text string looks like EXPLAIN output. */
export function isExplainText(text: string): boolean {
  const trimmed = text.trim();

  // JSON format
  if (trimmed.startsWith('[') || trimmed.startsWith('{')) {
    try {
      const parsed = JSON.parse(trimmed);
      if (Array.isArray(parsed) && parsed.length > 0 && parsed[0]?.Plan) {
        return true;
      }
      if (typeof parsed === 'object' && parsed !== null && 'Plan' in parsed) {
        return true;
      }
    } catch {
      // Not valid JSON, fall through
    }
  }

  // Text format heuristics
  if (/\(cost=\d/.test(trimmed)) return true;

  // Known node type prefixes at the start of an EXPLAIN line
  const nodePatterns = [
    'Seq Scan',
    'Index Scan',
    'Index Only Scan',
    'Bitmap',
    'Hash Join',
    'Merge Join',
    'Nested Loop',
    'Sort',
    'Aggregate',
    'Group',
    'Limit',
    'Append',
    'Unique',
    'Materialize',
    'Subquery Scan',
    'CTE Scan',
    'Function Scan',
    'Values Scan',
    'Result',
    'WindowAgg',
    'HashAggregate',
    'GroupAggregate',
    'Gather',
    'Gather Merge',
    'Parallel',
  ];

  for (const pattern of nodePatterns) {
    if (trimmed.startsWith(pattern)) return true;
  }

  return false;
}

export function isExplainResult(
  columns: { name: string }[],
  cells: unknown[],
): boolean {
  if (columns.length !== 1) return false;

  const colName = columns[0].name.toUpperCase();
  if (colName !== 'QUERY PLAN') return false;

  if (cells.length === 0) return true;

  const firstCell = cells[0];
  let text: string;

  if (typeof firstCell === 'string') {
    text = firstCell;
  } else if (typeof firstCell === 'object' && firstCell !== null) {
    // Handle CellValue union type — look for Text variant
    if ('Text' in firstCell) {
      text = (firstCell as { Text: string }).Text;
    } else {
      return false;
    }
  } else {
    return false;
  }

  return isExplainText(text);
}
