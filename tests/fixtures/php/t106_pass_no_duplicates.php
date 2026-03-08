<?php

use PHPUnit\Framework\TestCase;

class NoDuplicateTest extends TestCase
{
    public function test_different_literals_in_assertions(): void
    {
        $this->assertEquals(10, calculate(1));
        $this->assertEquals(20, calculate(2));
        $this->assertEquals(30, calculate(3));
    }
}
