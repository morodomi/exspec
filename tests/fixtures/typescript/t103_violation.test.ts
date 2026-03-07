describe('UserService', () => {
  it('creates a user', () => {
    const user = createUser('alice');
    expect(user.name).toBe('alice');
  });

  it('deletes a user', () => {
    const result = deleteUser(1);
    expect(result).toBe(true);
  });
});
