<?php

use PHPUnit\Framework\TestCase;

class AssertionWithMessagesTest extends TestCase
{
    public function test_multiple_asserts_with_messages(): void
    {
        $this->assertEquals(2, 1 + 1, "addition of ones");
        $this->assertEquals(4, 2 + 2, "addition of twos");
    }
}
