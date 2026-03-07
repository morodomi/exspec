describe.each([
  { input: 1, expected: 2 },
  { input: 2, expected: 4 },
])('doubler with $input', ({ input, expected }) => {
  let calculator: any;
  let logger: any;

  it('doubles correctly', () => {
    expect(input * 2).toBe(expected);
  });
});
