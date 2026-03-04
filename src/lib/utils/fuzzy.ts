export type FuzzyResult = {
  score: number;
  positions: number[];
};

function isWordBoundary(target: string, i: number): boolean {
  if (i === 0) return true;
  const prev = target[i - 1];
  const curr = target[i];
  // After separator
  if (prev === '_' || prev === '-' || prev === '.') return true;
  // camelCase transition
  if (prev >= 'a' && prev <= 'z' && curr >= 'A' && curr <= 'Z') return true;
  return false;
}

export function fuzzyMatch(query: string, target: string): FuzzyResult | null {
  if (!query) return { score: 0, positions: [] };

  const queryLower = query.toLowerCase();
  const targetLower = target.toLowerCase();
  const positions: number[] = [];
  let score = 0;
  let qi = 0;
  let lastMatchIndex = -1;

  for (let ti = 0; ti < targetLower.length && qi < queryLower.length; ti++) {
    if (targetLower[ti] === queryLower[qi]) {
      positions.push(ti);

      // Scoring
      if (ti === 0) {
        score += 3; // Start of string
      } else if (isWordBoundary(target, ti)) {
        score += 2; // Word boundary
      }

      if (lastMatchIndex >= 0 && ti === lastMatchIndex + 1) {
        score += 1; // Consecutive
      } else if (lastMatchIndex >= 0) {
        score -= 0.5 * (ti - lastMatchIndex - 1); // Gap penalty
      }

      lastMatchIndex = ti;
      qi++;
    }
  }

  // All query chars must be found
  if (qi < queryLower.length) return null;

  return { score, positions };
}
