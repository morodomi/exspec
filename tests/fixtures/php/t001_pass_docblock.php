<?php

class UserTest extends TestCase
{
    /** @test */
    public function creates_a_user(): void
    {
        $user = new User("test");
        $this->assertTrue($user->isValid());
    }
}
