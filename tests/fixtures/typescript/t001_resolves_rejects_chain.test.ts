import { describe, it, expect } from 'vitest';

describe('modifier chains', () => {
  it('should detect resolves.toBe', async () => {
    await expect(Promise.resolve(42)).resolves.toBe(42);
  });

  it('should detect resolves.not.toThrow', async () => {
    await expect(Promise.resolve()).resolves.not.toThrow();
  });

  it('should detect rejects.not.toThrow', async () => {
    await expect(Promise.reject(new Error())).rejects.not.toThrow(TypeError);
  });
});
