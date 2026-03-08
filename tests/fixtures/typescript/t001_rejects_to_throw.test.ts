import { describe, it, expect } from 'vitest';

describe('async error handling', () => {
  it('should reject with error', async () => {
    await expect(fetchUser('')).rejects.toThrow('not found');
  });
});
