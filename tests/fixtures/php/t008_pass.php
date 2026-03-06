<?php

use Illuminate\Foundation\Http\FormRequest;

class UserTest extends TestCase
{
    public function test_create_user(): void
    {
        $user = new User("test");
        $this->assertEquals("test", $user->getName());
    }
}
