<?php

use PHPUnit\Framework\TestCase;

class NoSleepTest extends TestCase
{
    public function test_no_waiting(): void
    {
        $result = compute(42);
        $this->assertEquals(84, $result);
    }
}
