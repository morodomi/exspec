import { describe, it, expect } from 'vitest';

describe('poll assertions', () => {
  it('should eventually match', () => {
    expect.poll(() => fetchValue()).toBe(42);
  });
});
