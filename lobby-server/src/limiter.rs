use tower_governor::governor::GovernorConfigBuilder;

pub fn secure() -> tower_governor::governor::GovernorConfig<
    tower_governor::key_extractor::PeerIpKeyExtractor,
    governor::middleware::NoOpMiddleware<governor::clock::QuantaInstant>,
> {
    GovernorConfigBuilder::default()
        .per_second(4)
        .burst_size(2)
        .finish()
        .unwrap()
}

pub async fn cleanup_limiter_task(cleanup: impl Fn()) -> ! {
    use tokio::time::Duration;

    const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
    loop {
        tokio::time::sleep(CLEANUP_INTERVAL).await;
        cleanup();
    }
}
