<?php

class NamedClassAssertTest extends TestCase
{
    public function test_assert_class_equals(): void
    {
        $result = Calculator::add(1, 2);
        Assert::assertEquals(3, $result);
    }

    public function test_fqcn_assert_same(): void
    {
        $value = Service::compute();
        PHPUnit\Framework\Assert::assertSame('expected', $value);
    }

    public function test_assert_class_true(): void
    {
        $flag = FeatureFlag::isEnabled('beta');
        Assert::assertTrue($flag);
    }
}
