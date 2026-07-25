const DEFAULT_SEED = 0x70676c31;

export class SeededRandom {
  private state: number;

  public constructor(seed: number = DEFAULT_SEED) {
    this.state = normalizeSeed(seed);
  }

  public reset(seed: number = DEFAULT_SEED): void {
    this.state = normalizeSeed(seed);
  }

  public next(): number {
    let value = this.state;
    value ^= value << 13;
    value ^= value >>> 17;
    value ^= value << 5;
    this.state = value >>> 0;
    return this.state / 0x1_0000_0000;
  }

  public between(a: number, b: number): number {
    return a + (b - a) * this.next();
  }
}

function normalizeSeed(seed: number): number {
  const normalized = Math.trunc(seed) >>> 0;
  return normalized === 0 ? DEFAULT_SEED : normalized;
}
