describe('UserService', () => {
  it('creates a user', () => {
    const user = createUser('alice');
    expect(user.name).toBe('alice');
  });

  it('throws specific error on empty name', () => {
    expect(() => createUser('')).toThrowError('name is required');
  });
});
