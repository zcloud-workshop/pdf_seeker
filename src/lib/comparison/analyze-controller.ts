/**
 * Compare analysis controller: binds one analysis to a file pair plus a
 * monotonically increasing request token. Only the latest request may write
 * texts, errors and the analyzing flag; results from older requests (or
 * their finally blocks) are discarded. dispose() cleans up the diff engine
 * and voids any in-flight request.
 */

import type { DiffRunOutcome, PageDiff } from "./page-diff";
import type { DiffService } from "./diff-service";

export interface FilePair {
  a: string;
  b: string;
}

export interface CompareState {
  analyzing: boolean;
  errorMsg: string;
  /** File pair the displayed data belongs to. */
  pair: FilePair | null;
  textsA: string[];
  textsB: string[];
  pages: PageDiff[];
  outcome: DiffRunOutcome | null;
}

export function initialCompareState(): CompareState {
  return {
    analyzing: false,
    errorMsg: "",
    pair: null,
    textsA: [],
    textsB: [],
    pages: [],
    outcome: null,
  };
}

export type ExtractFn = (path: string) => Promise<string[]>;

export interface CompareControllerDeps {
  extract: ExtractFn;
  diff: Pick<DiffService, "run" | "dispose">;
  onState: (state: CompareState) => void;
}

export class CompareController {
  private token = 0;
  private disposed = false;

  constructor(private readonly deps: CompareControllerDeps) {}

  run(pair: FilePair): void {
    const token = ++this.token;
    this.deps.onState({
      analyzing: true,
      errorMsg: "",
      pair,
      textsA: [],
      textsB: [],
      pages: [],
      outcome: null,
    });
    void this.analyze(pair, token);
  }

  dispose(): void {
    this.disposed = true;
    this.token++;
    this.deps.diff.dispose();
  }

  private async analyze(pair: FilePair, token: number): Promise<void> {
    let textsA: string[];
    let textsB: string[];
    try {
      const [ta, tb] = await Promise.all([
        this.deps.extract(pair.a),
        this.deps.extract(pair.b),
      ]);
      textsA = ta;
      textsB = tb;
    } catch (e) {
      if (this.stale(token)) return;
      this.deps.onState({
        analyzing: false,
        errorMsg: String(e),
        pair,
        textsA: [],
        textsB: [],
        pages: [],
        outcome: null,
      });
      return;
    }
    if (this.stale(token)) return;

    const outcome = await this.deps.diff.run(textsA, textsB);
    if (this.stale(token) || !outcome || outcome.canceled) return;
    this.deps.onState({
      analyzing: false,
      errorMsg: "",
      pair,
      textsA,
      textsB,
      pages: outcome.pages,
      outcome,
    });
  }

  private stale(token: number): boolean {
    return this.disposed || token !== this.token;
  }
}
