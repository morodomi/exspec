<?php

class MathTest extends TestCase
{
    public function test_add(): void
    {
        $result = 1 + 2;
        // no assertion
    }

    public function test_subtract(): void
    {
        $result = 3 - 2;
        $this->assertEquals(1, $result);
    }
}
