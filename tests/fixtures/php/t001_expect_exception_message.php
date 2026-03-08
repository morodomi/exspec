<?php

use PHPUnit\Framework\TestCase;

class UserMessageTest extends TestCase
{
    /** @test */
    public function it_shows_correct_error_message(): void
    {
        $this->expectExceptionMessage('Name cannot be empty');
        new User('');
    }
}
