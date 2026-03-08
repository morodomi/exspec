<?php

use PHPUnit\Framework\TestCase;

class UndescriptiveTest extends TestCase
{
    public function test_1(): void
    {
        $this->assertTrue(true);
    }

    public function test_it(): void
    {
        $this->assertTrue(true);
    }

    public function test_case(): void
    {
        $this->assertTrue(true);
    }
}
