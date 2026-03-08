import { describe, it, expectTypeOf } from 'vitest';

describe('type tests', () => {
  it('should have correct type', () => {
    expectTypeOf(createUser('Alice')).toEqualTypeOf<User>();
  });
});
