<?php

use PHPUnit\Framework\TestCase;

class UserTest extends TestCase
{
    /** @test */
    public function it_throws_on_invalid_input(): void
    {
        $this->expectException(\InvalidArgumentException::class);
        new User('');
    }
}
