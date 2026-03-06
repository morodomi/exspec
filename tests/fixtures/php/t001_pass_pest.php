<?php

test('creates a user', function () {
    $user = new User("test");
    expect($user->getName())->toBe("test");
});
