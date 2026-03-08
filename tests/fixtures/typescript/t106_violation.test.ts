test('duplicate literals in assertions', () => {
    expect(calculate(1)).toBe(42);
    expect(calculate(2)).toBe(42);
    expect(calculate(3)).toBe(42);
});
