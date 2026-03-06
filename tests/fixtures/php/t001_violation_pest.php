<?php

test('creates a user', function () {
    $user = new User("test");
    $user->getName();
});
