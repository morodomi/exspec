<?php

class SkipOnlyViolationTest extends TestCase
{
    public function testSkippedWithoutAssertion(): void
    {
        $this->markTestSkipped('Not supported yet.');
    }
}
