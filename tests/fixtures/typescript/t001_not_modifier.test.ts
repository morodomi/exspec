import { describe, it, expect } from 'vitest';

describe('not modifier', () => {
  it('should use not.toBe', () => {
    expect(1).not.toBe(2);
  });

  it('should use not.toEqual', () => {
    expect({ a: 1 }).not.toEqual({ a: 2 });
  });

  it('should use not.toContain', () => {
    expect([1, 2, 3]).not.toContain(4);
  });
});
