<?php

use PHPUnit\Framework\TestCase;

class AssertionRouletteTest extends TestCase
{
    public function test_multiple_asserts_no_messages(): void
    {
        $this->assertEquals(2, 1 + 1);
        $this->assertEquals(4, 2 + 2);
        $this->assertTrue(true);
    }
}
