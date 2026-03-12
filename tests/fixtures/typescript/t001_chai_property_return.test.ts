import { expect } from 'chai';

// Chai property-style assertions wrapped in return statements.
// These should count as assertions inside block-body test callbacks.

describe('chai property in return statements', () => {
  // TC-01: depth 1 — return expect(x).TERMINAL
  it('should detect return expect(value).ok', () => {
    return expect(value).ok;
  });

  // TC-02: depth 2 — return expect(x).chain.TERMINAL
  it('should detect return expect(spy).not.called', () => {
    return expect(spy).not.called;
  });

  // TC-03: depth 3 — return expect(x).a.b.TERMINAL
  it('should detect return expect(promise).to.be.rejected', () => {
    return expect(promise).to.be.rejected;
  });

  // TC-04: depth 4 — return expect(x).a.b.c.TERMINAL
  it('should detect return expect(spy).to.have.been.calledOnce', () => {
    return expect(spy).to.have.been.calledOnce;
  });

  // TC-05: depth 5 — return expect(x).a.b.c.d.TERMINAL
  it('should detect return expect(spy).to.not.have.been.calledOnce', () => {
    return expect(spy).to.not.have.been.calledOnce;
  });

  // TC-06: negative — non-assertion return must not be counted
  it('should not count non-assertion return', () => {
    return someNonAssertionExpr;
  });
});
