<?php

use PHPUnit\Framework\TestCase;

class DuplicateLiteralTest extends TestCase
{
    public function test_duplicate_literals_in_assertions(): void
    {
        $this->assertEquals(42, calculate(1));
        $this->assertEquals(42, calculate(2));
        $this->assertEquals(42, calculate(3));
    }
}
