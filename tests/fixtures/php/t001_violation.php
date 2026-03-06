<?php

class UserTest extends TestCase
{
    public function test_create_user(): void
    {
        $user = new User("test");
        $user->getName();
    }
}
