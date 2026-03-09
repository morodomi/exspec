<?php

class NonThisExpectsTest extends TestCase
{
    public function test_event_emitter_expects_not_assertion(): void
    {
        $emitter = new EventEmitter();
        $emitter->expects('click');
        $emitter->dispatch('click');
    }

    public function test_mock_expects_not_this(): void
    {
        $mock = $this->createMock(PaymentGateway::class);
        $mock->expects($this->once())
            ->method('charge')
            ->with(100);

        $service = new PaymentService($mock);
        $service->processPayment(100);
    }
}
