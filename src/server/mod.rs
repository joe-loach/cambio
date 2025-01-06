pub mod config;
mod connection;
mod data;
mod disconnect;
mod event;
mod player;

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::ControlFlow;
use std::sync::Arc;

use config::Config;
use connection::Connections;
use data::PlayerData;
pub use event::Event;
use itertools::Itertools;
use parking_lot::Mutex;
use player::PlayerConn;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::{net::TcpListener, select, time};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, trace, warn};

use crate::client;

type GameData = Arc<Mutex<data::GameData>>;

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
        let data = Arc::new(Mutex::new(Default::default()));
        let mut connections = Connections::default();

        select! {
            _ = self.game_sequence(&mut connections, data) => {}
            _ = token.cancelled() => {
                info!("server cancelled");
            }
        }

        self.close(&connections);
    }

    async fn game_sequence(&self, connections: &mut Connections, mut data: GameData) {
        self.lobby(connections, &mut data).await;

        self.setup(connections, &mut data).await;

        let mut rounds = 0;

        loop {
            connections.broadcast(Event::RoundStart(rounds));

            self.play_round(connections, &mut data, rounds).await;

            connections.broadcast(Event::RoundEnd);
            connections.broadcast(Event::ConfirmNewRound);

            if !self.new_round(connections, &data).await {
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

        let player_count = data.lock().player_count();
        let mut turn = (FIRST_PLAYER + round_offset) % player_count;

        loop {
            connections.broadcast(Event::TurnStart {
                uuid: data.lock().get_player(turn).id(),
            });

            let Some(card) = data.lock().deck.draw() else {
                connections.broadcast(Event::EndTurn);

                break;
            };
            connections.broadcast(Event::DrawCard(card));

            connections.broadcast(Event::WaitingForDecision);

            // read decision
            while let Some((id, event)) = connections.events().recv().await {
                if id == data.lock().get_player(turn).id() {
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

        let winner_result = {
            let data = data.lock();

            connections.broadcast(Event::ShowAll(data.players().to_vec()));

            let scores = data.players().iter().into_group_map_by(|p| p.score());
            let winner =
                scores
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

            if let Some(winner) = winner {
                let slot = data.players().iter().position(|p| p == *winner).unwrap();
                event::Winner::Player {
                    slot,
                    uuid: winner.id(),
                }
            } else {
                event::Winner::Tied
            }
        };

        connections.broadcast(Event::Winner(winner_result));
    }

    async fn new_round(&self, connections: &mut Connections, data: &GameData) -> bool {
        let responses_needed = data.lock().player_count();

        let responses = {
            let timeout = time::sleep(time::Duration::from_secs(self.config.new_round_timer_secs));
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

        responses >= responses_needed
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

    async fn setup(&self, connections: &mut Connections, data: &mut GameData) {
        connections.broadcast(Event::Starting);

        {
            trace!("shuffling cards");
            let mut rng = rand::thread_rng();
            data.lock().deck.shuffle(&mut rng);
        }

        connections.broadcast(Event::Setup);

        {
            let mut data = data.lock();

            for i in 0..data.player_count() {
                data::take_starting_cards(&mut data, i);
            }
        }

        connections.broadcast(Event::FirstDraw);

        connections
            .send_map(|id| {
                let data = data.lock();
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
}

impl GameServer {
    async fn lobby(&self, connections: &mut Connections, data: &mut GameData) {
        trace!("enter lobby");

        let listener = TcpListener::bind((
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            self.config.server_port,
        ))
        .await
        .expect("failed to create server port");

        info!("listening on {:?}", listener.local_addr().ok());

        let (shutdown, mut left) = disconnect::handler(data.clone());

        'waiting: loop {
            info!(
                "waiting for clients ({}/{})",
                data.lock().player_count(),
                config::MAX_PLAYER_COUNT
            );

            select! {
                // a client joined the lobby
                Ok((socket, addr)) = listener.accept() => {
                    let shutdown = shutdown.clone();
                    Self::accept_client(
                        socket,
                        addr,
                        data,
                        connections,
                        shutdown,
                    );
                }
                // listen for request to start game
                Some((id, event)) = connections.events().recv(),
                if data.lock().player_count() >= config::MIN_PLAYER_COUNT => {
                    if Self::try_start_game(id, event, data).is_break() {
                        break 'waiting
                    }
                }
                // a client left the lobby
                Some(id) = left.recv() => {
                    connections.broadcast(
                        Event::Left {
                            uuid: id,
                            player_count: data.lock().player_count()
                        }
                    );
                }
            };

            if data.lock().player_count() == config::MAX_PLAYER_COUNT {
                info!("max lobby capacity reached");
                break 'waiting;
            }
        }

        trace!("exiting lobby");
    }

    fn accept_client(
        socket: TcpStream,
        addr: SocketAddr,
        data: &mut GameData,
        connections: &mut Connections,
        shutdown: mpsc::Sender<(uuid::Uuid, player::CloseReason)>,
    ) {
        info!("new connection from {addr}");
        let player = PlayerData::new();
        let player_id = player.id();

        let mut data = data.lock();

        data.add_player(player);
        let player_count = data.player_count();

        drop(data);

        // spawn a player task
        player::spawn(connections, player_id, PlayerConn::from(socket), shutdown);

        // let everyone know someone has joined
        connections.broadcast(Event::Joined {
            uuid: player_id,
            player_count,
        });
    }

    fn try_start_game(id: uuid::Uuid, event: client::Event, data: &GameData) -> ControlFlow<()> {
        if let client::Event::Start = event {
            if host_id(data).is_some_and(|host| host == id) {
                info!("host started game");
                return ControlFlow::Break(());
            }
        } else {
            warn!(
                "player {id} in lobby gave another event {event:?} when expecting `Event::Start`"
            );
        }

        ControlFlow::Continue(())
    }
}

impl GameServer {
    fn close(&self, connections: &Connections) {
        connections.broadcast(Event::ServerClosing);
        connections.send_all(player::Command::Close);
    }
}

/// Gets the hosts uuid
fn host_id(data: &GameData) -> Option<uuid::Uuid> {
    data.lock().players().first().map(|p| p.id())
}
