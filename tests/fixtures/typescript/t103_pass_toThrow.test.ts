describe('UserService', () => {
  it('creates a user', () => {
    const user = createUser('alice');
    expect(user.name).toBe('alice');
  });

  it('throws on empty name', () => {
    expect(() => createUser('')).toThrow();
  });
});
