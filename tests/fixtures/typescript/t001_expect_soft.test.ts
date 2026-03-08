import { describe, it, expect } from 'vitest';

describe('soft assertions', () => {
  it('should collect all failures', () => {
    expect.soft(result.name).toBe('Alice');
    expect.soft(result.age).toBe(30);
  });
});
