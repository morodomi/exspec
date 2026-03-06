<?php

class SmallTest extends TestCase
{
    public function test_small(): void
    {
        $result = 1 + 2;
        $this->assertEquals(3, $result);
    }
}
