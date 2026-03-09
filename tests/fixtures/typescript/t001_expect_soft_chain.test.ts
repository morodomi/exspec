import { expect, describe, it } from 'vitest';

// expect.soft/element/poll modifier chain assertions.
// These are valid test oracles and should NOT trigger T001.

describe('expect.soft modifier chains', () => {
  // B1 (regression): expect.soft(x).toBe(y) — depth-2 existing
  it('should detect expect.soft depth-2', () => {
    expect.soft(value).toBe(42);
  });

  // B2: expect.soft(x).not.toBe(y) — depth-3
  it('should detect expect.soft.not depth-3', () => {
    expect.soft(value).not.toBe(42);
  });

  // B3: expect.soft(x).resolves.toBe(y) — depth-3
  it('should detect expect.soft.resolves depth-3', () => {
    expect.soft(promise).resolves.toBe(42);
  });

  // B4: expect.soft(x).rejects.toThrow() — depth-3
  it('should detect expect.soft.rejects depth-3', () => {
    expect.soft(promise).rejects.toThrow();
  });

  // B5: expect.soft(x).resolves.not.toBe(y) — depth-4
  it('should detect expect.soft.resolves.not depth-4', () => {
    expect.soft(promise).resolves.not.toBe(42);
  });

  // B6: expect.soft(x).rejects.not.toThrow(TypeError) — depth-4
  it('should detect expect.soft.rejects.not depth-4', () => {
    expect.soft(promise).rejects.not.toThrow(TypeError);
  });

  // B7 (negative): expect.soft(x).resolves.customHelper() — NOT a toX terminal
  it('should not detect customHelper as assertion', () => {
    expect.soft(value).resolves.customHelper();
  });

  // B8 (negative): no assertions
  it('should have no assertions', () => {
    const x = 1 + 1;
  });

  // B9: expect.element(loc).not.toHaveText('x') — depth-3
  it('should detect expect.element.not depth-3', () => {
    expect.element(locator).not.toHaveText('x');
  });

  // B10: expect.poll(fn).not.toBe(0) — depth-3
  it('should detect expect.poll.not depth-3', () => {
    expect.poll(() => count).not.toBe(0);
  });
});
