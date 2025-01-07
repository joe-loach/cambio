use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt, EnvFilter};

pub(crate) fn init() -> anyhow::Result<()> {
    fmt()
        .compact()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(if cfg!(debug_assertions) {
                    LevelFilter::TRACE.into()
                } else {
                    LevelFilter::ERROR.into()
                })
                .from_env_lossy(),
        )
        .try_init()
        .map_err(|e| anyhow::anyhow!(e))?;

    Ok(())
}
