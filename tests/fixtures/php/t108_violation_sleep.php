<?php

use PHPUnit\Framework\TestCase;

class SleepTest extends TestCase
{
    public function test_wait_for_result(): void
    {
        startTask();
        sleep(2);
        $this->assertEquals("done", getResult());
    }

    public function test_usleep_wait(): void
    {
        usleep(500000);
        $this->assertTrue(true);
    }
}
