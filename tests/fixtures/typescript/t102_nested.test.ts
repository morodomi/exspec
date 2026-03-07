describe('outer', () => {
  let db: any;
  let cache: any;
  let mailer: any;

  describe('inner', () => {
    let queue: any;
    let logger: any;
    let auth: any;

    it('test in nested describe inherits all fixtures', () => {
      expect(db).toBeDefined();
    });
  });

  it('test in outer describe only sees outer fixtures', () => {
    expect(db).toBeDefined();
  });
});
