import { expect } from 'chai';

// Chai BDD property-style assertions (no trailing parentheses).
// These are valid test oracles and should NOT trigger T001.
// Note: spy.calledOnce without expect() is NOT matched (no expect anchor).

describe('chai property assertions', () => {
  it('should detect ok (depth 1)', () => {
    expect(value).ok;
  });

  it('should detect to.be.true (depth 3)', () => {
    expect(value).to.be.true;
  });

  it('should detect to.be.null (depth 3)', () => {
    expect(value).to.be.null;
  });

  it('should detect to.have.been.calledOnce (depth 4)', () => {
    expect(spy).to.have.been.calledOnce;
  });

  it('should detect to.not.be.empty (depth 4)', () => {
    expect(list).to.not.be.empty;
  });
});
