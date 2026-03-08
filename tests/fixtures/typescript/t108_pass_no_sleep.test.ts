test('no waiting', () => {
    const result = compute(42);
    expect(result).toBe(84);
});
