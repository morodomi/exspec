test('waits with setTimeout', () => {
    setTimeout(() => {}, 1000);
    expect(true).toBe(true);
});

test('waits with sleep', async () => {
    await sleep(500);
    expect(result).toBe(true);
});
