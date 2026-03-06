<?php

use PHPUnit\Framework\Attributes\Test;

class UserTest extends TestCase
{
    #[\PHPUnit\Framework\Attributes\Test]
    public function creates_a_user(): void
    {
        $user = new User("test");
        $this->assertTrue($user->isValid());
    }
}
