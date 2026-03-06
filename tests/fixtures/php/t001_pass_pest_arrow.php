<?php

test('creates a user', fn() => expect(new User("test"))->toBeInstanceOf(User::class));
