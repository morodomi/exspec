// TypeScript: T107 should NOT fire (expect() has no message arg by design)
describe('assertion roulette pass', () => {
  test('multiple assertions without messages is normal in TS', () => {
    expect(1 + 1).toBe(2);
    expect(2 + 2).toBe(4);
    expect(3 + 3).toBe(6);
  });
});
