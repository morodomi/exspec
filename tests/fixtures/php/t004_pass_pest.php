<?php

test('add numbers', function (int $a, int $b, int $expected) {
    expect($a + $b)->toBe($expected);
})->with([
    [1, 2, 3],
    [4, 5, 9],
]);
