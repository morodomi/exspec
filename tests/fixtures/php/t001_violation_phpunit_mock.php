<?php

class PaymentTest extends TestCase
{
    public function test_charges_customer(): void
    {
        $mock = $this->createMock(PaymentGateway::class);
        $mock->expects($this->once())
            ->method('charge')
            ->with(100);

        $service = new PaymentService($mock);
        $service->processPayment(100);
    }
}
