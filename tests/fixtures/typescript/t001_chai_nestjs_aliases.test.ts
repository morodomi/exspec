import { expect } from 'chai';

// NestJS dogfooding: Chai alias/property vocabulary expansion (#50).
// TC-01 through TC-08 should be detected as assertions.
// TC-09 should NOT be detected (TP control).

describe('chai nestjs aliases', () => {
  // TC-01: instanceof method alias (depth-3)
  it('should detect instanceof alias', () => {
    expect(foo).to.be.instanceof(Bar);
  });

  // TC-02: throws method alias (depth-2)
  it('should detect throws alias', () => {
    expect(fn).to.throws(Error);
  });

  // TC-03: contains method alias (depth-2)
  it('should detect contains alias', () => {
    expect(arr).to.contains(item);
  });

  // TC-04: equals method alias (depth-2)
  it('should detect equals alias', () => {
    expect(x).to.equals(y);
  });

  // TC-05: ownProperty method (depth-3)
  it('should detect ownProperty', () => {
    expect(obj).to.have.ownProperty('key');
  });

  // TC-06: length method alias (depth-3)
  it('should detect length alias', () => {
    expect(arr).to.have.length(5);
  });

  // TC-07: throw property terminal (depth-3, no parens)
  it('should detect throw property', () => {
    expect(fn).to.be.throw;
  });

  // TC-08: and intermediate + instanceof alias (deep chain)
  it('should detect and intermediate with instanceof', () => {
    expect(promise).to.be.rejected.and.be.an.instanceof(Error);
  });

  // TC-09: negative — truly assertion-free (TP control)
  it('should have no assertions', () => {
    const x = 1 + 1;
  });
});
