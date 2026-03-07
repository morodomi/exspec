<?php

use PHPUnit\Framework\Attributes\DataProvider;
use PHPUnit\Framework\Attributes\Test;

class DataProviderTest extends \PHPUnit\Framework\TestCase
{
    public static function additionProvider(): array
    {
        return [[1, 2, 3], [0, 0, 0]];
    }

    #[DataProvider('additionProvider')]
    public function test_addition(int $a, int $b, int $expected): void
    {
        $this->assertEquals($expected, $a + $b);
    }

    #[DataProvider('additionProvider')]
    #[Test]
    public function addition_with_test_attr(int $a, int $b, int $expected): void
    {
        $this->assertEquals($expected, $a + $b);
    }

    // No DataProvider — params ARE fixtures
    public function test_with_fixtures($db, $cache, $logger, $mailer, $queue, $config): void
    {
        $this->assertTrue(true);
    }
}
