pub(crate) enum Args {
    Server,
    Client,
}

pub(crate) fn parse_args() -> anyhow::Result<Args> {
    let mut pargs = pico_args::Arguments::from_env();

    match pargs.subcommand()?.as_deref() {
        Some("server") => Ok(Args::Server),
        Some("client") => Ok(Args::Client),
        _ => {
            anyhow::bail!("must supply either 'server' or 'client'")
        }
    }
}
