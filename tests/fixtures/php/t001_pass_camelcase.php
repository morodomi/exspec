<?php

class UserTest extends TestCase
{
    public function testCreateUser(): void
    {
        $user = new User("test");
        $this->assertEquals("test", $user->getName());
    }
}
