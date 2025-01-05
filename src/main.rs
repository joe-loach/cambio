mod cli;
mod log;

use std::net::IpAddr;

use cambio::{
    client::GameClient,
    server::{self, GameServer},
};
use tokio::{select, task};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

async fn start_client() -> anyhow::Result<()> {
    let token = CancellationToken::new();

    let client_task = {
        let token = token.child_token();

        let addr = (
            "127.0.0.1".parse::<IpAddr>().unwrap(),
            server::config::defaults::port(),
        );

        task::spawn(async move {
            let client = GameClient::connect(addr).await;
            client.start(token).await;
        })
    };

    tokio::pin!(client_task);

    let shutdown = tokio::signal::ctrl_c();

    select! {
        Ok(_) = shutdown => {
            info!("shutdown signal received");
            token.cancel();
            if let Err(e) = client_task.await {
                error!("client task error: {e}");
            }
        }
        res = &mut client_task => {
            if let Err(e) = res {
                error!("client task error: {e}");
            }
        }
        else => {}
    }

    Ok(())
}

async fn start_server() -> anyhow::Result<()> {
    let token = CancellationToken::new();

    let server_task = {
        let token = token.child_token();

        task::spawn(async move {
            let server = GameServer::from_config();

            server.run(token).await;
        })
    };

    tokio::pin!(server_task);

    let ctrl_c = tokio::signal::ctrl_c();

    select! {
        Ok(_) = ctrl_c => {
            info!("shutdown signal received");
            token.cancel();
            if let Err(e) = server_task.await {
                error!("server task error: {e}");
            }
        }
        res = &mut server_task => {
            if let Err(e) = res {
                error!("server task error: {e}");
            }
        }
        else => {}
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    log::init()?;

    match cli::parse_args()? {
        cli::Args::Server => start_server().await?,
        cli::Args::Client => start_client().await?,
    }

    Ok(())
}
