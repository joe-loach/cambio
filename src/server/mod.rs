pub mod config;
mod connection;
mod data;
mod event;
mod player;

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};

use config::Config;
use connection::Connections;
use data::GameData;
pub use event::Event;
use itertools::Itertools;
use player::{PlayerConn, PlayerData};
use tokio::{net::TcpListener, select, time};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, trace};

use crate::client;

pub struct GameServer {
    config: Config,
}

impl GameServer {
    pub fn from_config() -> Self {
        let config = match config::load() {
            Ok(cfg) => cfg,
            Err(e) => {
                error!("error loading config: {e}");
                info!("using default config");
                Config::default()
            }
        };

        GameServer { config }
    }

    pub async fn run(&self, token: CancellationToken) {
        let mut data = GameData::initial();
        let mut connections = Connections::default();

        select! {
            _ = self.game_sequence(&mut data, &mut connections) => {}
            _ = token.cancelled() => {
                info!("server cancelled");
            }
        }

        self.close(&connections);
    }

    async fn game_sequence(&self, data: &mut GameData, connections: &mut Connections) {
        self.lobby(connections, data).await;

        self.setup(connections, data).await;

        let mut rounds = 0;

        loop {
            connections.broadcast(Event::RoundStart(rounds));

            self.play_round(connections, data, rounds).await;

            connections.broadcast(Event::RoundEnd);
            connections.broadcast(Event::ConfirmNewRound);

            let responses_needed = data.player_count();
            let responses = {
                let timeout =
                    time::sleep(time::Duration::from_secs(self.config.new_round_timer_secs));
                tokio::pin!(timeout);

                let mut responses = HashSet::new();

                loop {
                    select! {
                        _ = &mut timeout => { info!("new round time out"); break; }
                        Some((id, client::Event::Continue)) = connections.events().recv() => {
                            responses.insert(id);

                            if responses.len() == responses_needed {
                                break;
                            }
                        }
                        else => break,
                    }
                }

                responses.len()
            };

            if responses < responses_needed {
                connections.broadcast(Event::GameEnd);
                break;
            }

            rounds += 1;
        }
    }
}

impl GameServer {
    async fn play_round(
        &self,
        connections: &mut Connections,
        data: &mut GameData,
        round_offset: usize,
    ) {
        const FIRST_PLAYER: usize = 0;

        let player_count = data.player_count();
        let mut turn = (FIRST_PLAYER + round_offset) % player_count;

        loop {
            connections.broadcast(Event::TurnStart {
                slot: turn,
                uuid: data.get_player(turn).id(),
            });

            let Some(card) = data.deck.draw() else {
                connections.broadcast(Event::EndTurn);

                break;
            };
            connections.broadcast(Event::DrawCard(card));

            connections.broadcast(Event::WaitingForDecision);

            // read decision
            while let Some((id, event)) = connections.events().recv().await {
                if id == data.get_player(turn).id() {
                    if let client::Event::Decision = event {
                        break;
                    }
                }
            }

            connections.broadcast(Event::PlayAction);

            self.listen_for_snaps(connections).await;

            connections.broadcast(Event::EndTurn);

            turn = (turn + 1) % player_count;
        }

        connections.broadcast(Event::CambioCall);

        {
            let cooldown = time::Duration::from_secs(self.config.show_all_cooldown);
            time::sleep(cooldown).await;
        }

        connections.broadcast(Event::ShowAll(data.players().to_vec()));

        let scores = data.players().iter().into_group_map_by(|p| p.score());
        let winner = scores
            .iter()
            .min_by(|(a, _), (b, _)| a.cmp(b))
            .and_then(|(_, players)| {
                if let [winner] = players.as_slice() {
                    Some(winner)
                } else {
                    // no sole winner
                    None
                }
            });

        let winner_result = if let Some(winner) = winner {
            let slot = data.players().iter().position(|p| p == *winner).unwrap();
            event::Winner::Player {
                slot,
                uuid: winner.id(),
            }
        } else {
            event::Winner::Tied
        };

        connections.broadcast(Event::Winner(winner_result));
    }

    async fn setup(&self, connections: &mut Connections, data: &mut GameData) {
        connections.broadcast(Event::Starting);

        {
            trace!("shuffling cards");
            let mut rng = rand::thread_rng();
            data.deck.shuffle(&mut rng);
        }

        connections.broadcast(Event::Setup);

        for i in 0..data.player_count() {
            data::take_starting_cards(data, i);
        }

        connections.broadcast(Event::FirstDraw);

        connections
            .send_map(|id| {
                let p = data
                    .players()
                    .iter()
                    .find(|p| p.id() == id)
                    .expect("player no longer exists");
                let [a, b] = p.cards()[..2] else {
                    unreachable!()
                };
                Event::FirstPeek(a, b)
            })
            .await;
    }

    async fn lobby(&self, connections: &mut Connections, data: &mut GameData) {
        trace!("enter lobby");

        let listener = TcpListener::bind((
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            self.config.server_port,
        ))
        .await
        .expect("failed to create server port");

        info!("listening on {:?}", listener.local_addr().ok());

        let (notify_shutdown, mut shutdown) = tokio::sync::mpsc::channel(config::MAX_PLAYER_COUNT);
        let mut host = None;

        'waiting: loop {
            info!(
                "waiting for clients ({}/{})",
                data.player_count(),
                config::MAX_PLAYER_COUNT
            );

            let (socket, addr) = {
                // either accept client or a request to start the game
                select! {
                    Ok(client) = listener.accept() => { client }

                    Some((id, client::Event::Start)) = connections.events().recv(),
                    if data.player_count() >= config::MIN_PLAYER_COUNT => {
                        if host.is_some_and(|host| host == id) {
                            info!("host started game");
                            break 'waiting;
                        } else {
                            continue 'waiting;
                        }
                    }

                    else => { continue 'waiting; }
                }
            };

            info!("new connection from {addr}");
            let player = PlayerData::new();
            let player_id = player.id();

            if data.player_count() == 0 {
                // the first player becomes the host
                host = Some(player_id);
            }

            let slot = data.add_player(player);

            // spawn a player task
            player::spawn(
                connections,
                player_id,
                PlayerConn::from(socket),
                notify_shutdown.clone(),
            );

            // let everyone know someone has joined
            let player_count = data.player_count();

            connections.broadcast(Event::Joined {
                slot,
                uuid: player_id,
                player_count,
            });

            if player_count == config::MAX_PLAYER_COUNT {
                info!("max lobby capacity reached");
                break 'waiting;
            }
        }

        trace!("exiting lobby");
    }

    async fn listen_for_snaps(&self, connections: &mut Connections) -> Option<uuid::Uuid> {
        connections.broadcast(Event::WaitingForSnap);

        let timeout = time::sleep(time::Duration::from_secs(self.config.snap_time_secs));
        tokio::pin!(timeout);

        loop {
            select! {
                Some((id, event)) = connections.events().recv() => {
                    if let client::Event::Snap = event {
                        info!("Player {id} snapped");
                        break Some(id);
                    }
                }
                _ = &mut timeout => {
                    info!("snap time out");
                    break None;
                }
            }
        }
    }
}

impl GameServer {
    fn close(&self, connections: &Connections) {
        connections.broadcast(Event::ServerClosing);
        connections.send_all(player::Command::Close);
    }
}
