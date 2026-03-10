<?php

class FacadeAssertTest extends TestCase
{
    public function test_event_assert_dispatched(): void
    {
        Event::fake();
        event(new OrderCreated());
        Event::assertDispatched(OrderCreated::class);
    }

    public function test_sleep_assert_sequence(): void
    {
        Sleep::fake();
        Sleep::for(5)->seconds();
        Sleep::assertSequence([
            Sleep::for(5)->seconds(),
        ]);
    }

    public function test_exceptions_assert_reported(): void
    {
        Exceptions::fake();
        Exceptions::report(new RuntimeException('test'));
        Exceptions::assertReported(RuntimeException::class);
    }

    public function test_bus_assert_dispatched(): void
    {
        Bus::fake();
        dispatch(new ProcessOrder());
        Bus::assertDispatched(ProcessOrder::class);
    }
}
