<?php
// PHP PBT is not mature, so T005 always triggers.
// This file exists for symmetry but will always report T005.
// If a PHP PBT library emerges, update import_pbt.scm query.

class MathTest extends TestCase
{
    public function test_add(): void
    {
        $this->assertEquals(3, 1 + 2);
    }
}
