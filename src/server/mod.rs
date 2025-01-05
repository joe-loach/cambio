pub mod config;
mod data;
mod event;

use std::net::{IpAddr, Ipv4Addr};

use config::Config;
use dashmap::DashMap;
use data::GameData;
pub use event::Event;
use futures::SinkExt;
use itertools::Itertools;
use tokio::{net::TcpListener, select, time};
use tokio_stream::StreamExt;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, trace};

use crate::{
    client,
    player::{PlayerConn, PlayerData},
};

pub struct GameServer {
    config: Config,
}

type ConnectionMap = DashMap<uuid::Uuid, PlayerConn>;

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
        let mut connections = ConnectionMap::new();

        select! {
            _ = self.game_sequence(&mut data, &mut connections) => {}
            _ = token.cancelled() => {
                info!("server cancelled");
                self.close(&mut connections, &data.players).await;
            }
        }
    }

    async fn game_sequence(&self, data: &mut GameData, connections: &mut ConnectionMap) {
        self.lobby(connections, data).await;

        self.setup(connections, data).await;

        let mut rounds = 0;

        loop {
            self.broadcast(connections, Event::RoundStart(rounds), &data.players)
                .await;

            self.play_round(connections, data, rounds).await;

            self.broadcast(connections, Event::RoundEnd, &data.players)
                .await;

            self.broadcast(connections, Event::ConfirmNewRound, &data.players)
                .await;

            let responses_needed = data.players.len();
            let responses = {
                let mut cons = data
                    .players
                    .iter_mut()
                    .map(|p| connections.get_mut(&p.id()).unwrap())
                    .collect::<Vec<_>>();

                let mut listeners = futures::stream::SelectAll::new();
                for con in &mut cons {
                    listeners.push(&mut con.read);
                }

                let timeout =
                    time::sleep(time::Duration::from_secs(self.config.new_round_timer_secs));
                tokio::pin!(timeout);

                let mut responses = 0;

                loop {
                    select! {
                        _ = &mut timeout => { info!("new round time out"); break; }
                        Ok(Some(client::Event::Continue)) = listeners.try_next() => {
                            responses += 1;

                            if responses == responses_needed {
                                break;
                            }
                        }
                        else => break,
                    }
                }
                responses
            };

            if responses < responses_needed {
                self.broadcast(connections, Event::GameEnd, &data.players)
                    .await;
                break;
            }

            rounds += 1;
        }

        self.broadcast(connections, Event::ServerClosing, &data.players)
            .await;
    }
}

impl GameServer {
    async fn play_round(
        &self,
        connections: &mut ConnectionMap,
        data: &mut GameData,
        round_offset: usize,
    ) {
        const FIRST_PLAYER: usize = 0;

        let player_count = data.players.len();
        let mut turn = (FIRST_PLAYER + round_offset) % player_count;

        loop {
            self.broadcast(
                connections,
                Event::TurnStart {
                    slot: turn,
                    uuid: data.player(turn).id(),
                },
                &data.players,
            )
            .await;

            let Some(card) = data.deck.draw() else {
                self.broadcast(connections, Event::EndTurn, &data.players)
                    .await;

                break;
            };
            self.send(connections, Event::DrawCard(card), data.player_mut(turn))
                .await;

            self.broadcast(connections, Event::WaitingForDecision, &data.players)
                .await;

            // read decision
            let _ack = connections
                .get_mut(&data.player(turn).id())
                .unwrap()
                .read
                .try_next()
                .await
                .unwrap();

            self.broadcast(connections, Event::PlayAction, &data.players)
                .await;

            self.listen_for_snaps(connections, &mut data.players).await;

            self.broadcast(connections, Event::EndTurn, &data.players)
                .await;

            turn = (turn + 1) % player_count;
        }

        self.broadcast(connections, Event::CambioCall, &data.players)
            .await;

        {
            let cooldown = time::Duration::from_secs(self.config.show_all_cooldown);
            time::sleep(cooldown).await;
        }

        self.broadcast(
            connections,
            Event::ShowAll(data.players.clone()),
            &data.players,
        )
        .await;

        let scores = data.players.iter().into_group_map_by(|p| p.score());
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
            let slot = data.players.iter().position(|p| p == *winner).unwrap();
            event::Winner::Player {
                slot,
                uuid: winner.id(),
            }
        } else {
            event::Winner::Tied
        };

        self.broadcast(connections, Event::Winner(winner_result), &data.players)
            .await;
    }

    async fn setup(&self, connections: &mut ConnectionMap, data: &mut GameData) {
        self.broadcast(connections, Event::Starting, &data.players)
            .await;

        {
            trace!("shuffling cards");
            let mut rng = rand::thread_rng();
            data.deck.shuffle(&mut rng);
        }

        self.broadcast(connections, Event::Setup, &data.players)
            .await;

        for p in &mut data.players {
            crate::take_starting_cards(p, &mut data.deck);
        }

        self.broadcast(connections, Event::FirstDraw, &data.players)
            .await;

        self.send_map(
            connections,
            |p| {
                let [a, b] = p.cards()[..2] else {
                    unreachable!()
                };

                Event::FirstPeek(a, b)
            },
            &data.players,
        )
        .await;
    }

    async fn lobby(&self, connections: &mut ConnectionMap, data: &mut GameData) {
        trace!("enter lobby");

        let listener = TcpListener::bind((
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            self.config.server_port,
        ))
        .await
        .expect("failed to create server port");

        info!("listening on {:?}", listener.local_addr().ok());

        let mut host = None;

        'waiting: loop {
            info!(
                "waiting for clients ({}/{})",
                data.players.len(),
                config::MAX_PLAYER_COUNT
            );

            let (socket, addr) = {
                let mut cons = data
                    .players
                    .iter_mut()
                    .map(|p| (p.id(), connections.get_mut(&p.id()).unwrap()))
                    .collect::<Vec<_>>();

                let mut listeners = tokio_stream::StreamMap::new();
                for (id, stream) in &mut cons {
                    listeners.insert(*id, &mut stream.read);
                }

                // either accept client or a request to start the game
                select! {
                    Ok(client) = listener.accept() => { client }

                    Some((id, Ok(client::Event::Start))) = listeners.next(),
                        if data.players.len() >= config::MIN_PLAYER_COUNT =>
                    {
                        if host.is_some_and(|host| host == id) {
                            info!("host started game");
                            break 'waiting;
                        } else {
                            continue 'waiting;
                        }
                    }
                }
            };

            info!("new connection from {addr}");
            let player = PlayerData::new();
            let player_id = player.id();

            if data.players.is_empty() {
                // the first player becomes the host
                host = Some(player_id);
            }

            let slot = data.add_player(player);
            connections.insert(player_id, PlayerConn::from(socket));

            // let everyone know someone has joined
            let capacity = data.players.len();

            self.broadcast(
                connections,
                Event::Joined {
                    slot,
                    uuid: player_id,
                    capacity,
                },
                &data.players,
            )
            .await;

            if capacity == config::MAX_PLAYER_COUNT {
                info!("max lobby capacity reached");
                break 'waiting;
            }
        }

        trace!("exiting lobby");
    }

    async fn listen_for_snaps(
        &self,
        connections: &mut ConnectionMap,
        players: &mut [PlayerData],
    ) -> Option<uuid::Uuid> {
        self.broadcast(connections, Event::WaitingForSnap, players)
            .await;

        let mut cons = players
            .iter_mut()
            .map(|p| (p.id(), connections.get_mut(&p.id()).unwrap()))
            .collect::<Vec<_>>();

        let mut listeners = tokio_stream::StreamMap::new();
        for (id, stream) in &mut cons {
            listeners.insert(*id, &mut stream.read);
        }

        let timeout = time::sleep(time::Duration::from_secs(self.config.snap_time_secs));
        tokio::pin!(timeout);

        loop {
            select! {
                _ = &mut timeout => {
                    info!("snap time out");
                    break None;
                }
                res = listeners.next() => {
                    match res {
                        Some((id, Ok(client::Event::Snap))) => {
                            info!("Player {id} snapped");
                            break Some(id);
                        }
                        Some((_, Ok(_))) => {
                            // not a snap
                        }
                        Some((_, Err(e))) => {
                            error!("{e}");
                            break None;
                        }
                        None => {}
                    }
                }
            }
        }
    }
}

impl GameServer {
    async fn close(&self, connections: &mut ConnectionMap, players: &[PlayerData]) {
        self.broadcast(connections, Event::ServerClosing, players)
            .await;
    }
}

impl GameServer {
    async fn send(&self, connections: &mut ConnectionMap, event: Event, player: &PlayerData) {
        trace!("send stage: {:?} to player {}", event, player.id());
        self.inner_send(connections, event, player).await;
    }

    async fn send_map<F>(&self, connections: &mut ConnectionMap, f: F, players: &[PlayerData])
    where
        F: Fn(&PlayerData) -> Event,
    {
        for player in players {
            let event = f(player);
            self.inner_send(connections, event, player).await;
        }
    }

    async fn broadcast(
        &self,
        connections: &mut ConnectionMap,
        event: Event,
        players: &[PlayerData],
    ) {
        trace!("broadcasting stage: {:?}", &event);
        for player in players {
            self.inner_send(connections, event.clone(), player).await;
        }
    }

    async fn inner_send(&self, connections: &mut ConnectionMap, event: Event, player: &PlayerData) {
        let write = &mut connections.get_mut(&player.id()).unwrap().write;
        let res = write.send(event).await;
        if let Err(e) = res {
            error!("Error {e:?} when sending to player {}", player.id());
        }
    }
}
