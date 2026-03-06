<?php

class MathTest extends TestCase
{
    public function test_add(): void
    {
        $this->assertEquals(3, 1 + 2);
    }

    public function test_subtract(): void
    {
        $this->assertEquals(1, 3 - 2);
    }

    private function helper(): int
    {
        return 42;
    }

    public function test_multiply(): void
    {
        $this->assertEquals(6, 2 * 3);
    }
}
