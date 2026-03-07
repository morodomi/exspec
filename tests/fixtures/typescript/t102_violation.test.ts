describe('UserService', () => {
  let db: any;
  let cache: any;
  let mailer: any;
  let queue: any;
  let logger: any;
  let auth: any;

  beforeEach(() => {
    db = {};
    cache = {};
    mailer = {};
    queue = {};
    logger = {};
    auth = {};
  });

  it('creates user with too many fixtures', () => {
    expect(db).toBeDefined();
  });
});
