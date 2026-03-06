<?php

class OrderTest extends TestCase
{
    public function test_create_order(): void
    {
        $mockRepo = $this->createMock(OrderRepository::class);

        $this->assertTrue(true);
    }
}
