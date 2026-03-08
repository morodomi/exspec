use std::thread;
use std::time::Duration;

#[test]
fn test_wait_for_result() {
    start_task();
    thread::sleep(Duration::from_secs(2));
    assert_eq!(get_result(), "done");
}

#[tokio::test]
async fn test_async_wait() {
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(true);
}
