<?php

class MathTest extends TestCase
{
    #[DataProvider('additionProvider')]
    public function test_add(int $a, int $b, int $expected): void
    {
        $this->assertEquals($expected, $a + $b);
    }

    public static function additionProvider(): array
    {
        return [
            [1, 2, 3],
            [4, 5, 9],
        ];
    }

    public function test_subtract(): void
    {
        $this->assertEquals(1, 3 - 2);
    }
}
