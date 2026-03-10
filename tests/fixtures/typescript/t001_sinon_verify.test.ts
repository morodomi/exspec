import * as sinon from 'sinon';

// Sinon mock .verify() method-call oracle tests (#51).
// TC-01 through TC-05 should be detected as assertions.
// TC-06 and TC-07 should NOT be detected.

describe('sinon mock verify', () => {
  // TC-01: basic expectation.verify()
  it('should detect expectation.verify()', () => {
    const mock = sinon.mock(obj);
    const expectation = mock.expects('method').once();
    doSomething();
    expectation.verify();
  });

  // TC-02: mock.verify() directly
  it('should detect mock.verify()', () => {
    const mockObj = sinon.mock(container);
    mockObj.expects('addProvider').twice();
    await scanner.scan(TestModule);
    mockObj.verify();
  });

  // TC-03: chained expects + verify
  it('should detect chained mock.expects().verify()', () => {
    const mock = sinon.mock(obj);
    mock.expects('foo').once();
    mock.expects('bar').twice();
    doWork();
    mock.verify();
  });

  // TC-04: inline mock().expects().verify() — single chain
  it('should detect inline chain verify', () => {
    sinon.mock(obj).expects('method').once().verify();
  });

  // TC-05: verify with expect assertion — both should count
  it('should detect verify alongside expect', () => {
    const mock = sinon.mock(obj);
    mock.expects('method').once();
    doSomething();
    mock.verify();
    expect(result).to.equal(42);
  });

  // TC-06: negative — mock.restore() is NOT an assertion
  it('should not count mock.restore()', () => {
    const mock = sinon.mock(obj);
    mock.restore();
  });

  // TC-07: negative — no assertion
  it('should have no assertions', () => {
    const stub = sinon.stub(obj, 'method');
    stub.returns(42);
  });
});
