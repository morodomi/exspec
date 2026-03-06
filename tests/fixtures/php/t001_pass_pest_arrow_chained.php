<?php

test('adds numbers', fn(int $a, int $b, int $sum) => expect($a + $b)->toBe($sum))->with([
    [1, 2, 3],
    [10, 20, 30],
]);
