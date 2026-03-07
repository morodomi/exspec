describe('UserService', () => {
  it('creates a user', () => {
    const user = createUser('alice');
    expect(user.name).toBe('alice');
  });

  it('rejects on invalid input', async () => {
    await expect(createUserAsync('')).rejects.toThrow();
  });
});
