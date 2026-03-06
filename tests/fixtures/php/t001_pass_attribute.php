<?php

class UserTest extends TestCase
{
    #[Test]
    public function createUser(): void
    {
        $user = new User("test");
        $this->assertTrue($user->isValid());
    }
}
