import time

def test_wait_for_result():
    start_task()
    time.sleep(2)
    assert get_result() == "done"

import asyncio

async def test_async_wait():
    await asyncio.sleep(1)
    assert True
