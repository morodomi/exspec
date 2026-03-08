import { describe, it, expect } from 'vitest';

describe('element assertions', () => {
  it('should have correct text', () => {
    expect.element(locator).toHaveText('Hello');
  });
});
