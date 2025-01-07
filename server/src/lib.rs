pub mod config;
mod connection;
mod disconnect;
mod player;

use std::collections::HashSet;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::ControlFlow;
use std::sync::Arc;

use common::event::client;
use common::event::server::{Event, Winner};
use common::Deck;
use config::Config;
use connection::Connections;
use itertools::Itertools;
use parking_lot::Mutex;
use player::PlayerConn;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::{net::TcpListener, select, time};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, trace, warn};

use common::data::{self, PlayerData, Stage};

type GameData = Arc<Mutex<data::GameData>>;

pub enum Interrupt {
    Restart,
}

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
        let connections = Connections::default();

        let (ir_tx, mut interrupts) = mpsc::channel::<Interrupt>(8);
        let (shutdown, leaving, handler) = disconnect::handler(data.clone(), ir_tx);

        let (mut stop_game, game) = self.create_game(data, connections, shutdown.clone(), leaving);

        tokio::pin!(game);

        loop {
            select! {
                (_, connections, _) = &mut game => {
                    // game completed normally,
                    // close all connections
                    self.close(&connections);
                    break;
                },
                Some(ir) = interrupts.recv() => {
                    match ir {
                        Interrupt::Restart => {
                            // get old data
                            stop_game.send(()).await.expect("failed to cancel game");
                            let (data, connections, leaving) = (&mut game).await;
                            // let everyone know we're restarting
                            connections.broadcast(Event::Restart);
                            // create new game using old data
                            let (stop, fut) = self.create_game(
                                data,
                                connections,
                                shutdown.clone(),
                                leaving,
                            );
                            stop_game = stop;
                            // overwrite the future
                            game.set(fut);
                        }
                    }
                }
                _ = token.cancelled() => {
                    // abort, we don't care about cleaning up
                    // stop running the game loop
                    stop_game.send(()).await.expect("failed to cancel game");
                    // await it's finish
                    let _ = game.await;
                    break;
                },
            }
        }

        handler.abort();
    }

    fn create_game(
        &self,
        mut data: GameData,
        mut connections: Connections,
        shutdown: mpsc::Sender<(uuid::Uuid, player::CloseReason)>,
        mut leaving: mpsc::Receiver<uuid::Uuid>,
    ) -> (
        mpsc::Sender<()>,
        impl Future<Output = (GameData, Connections, mpsc::Receiver<uuid::Uuid>)> + use<'_>,
    ) {
        let (stop_tx, mut stop_rx) = mpsc::channel::<()>(8);

        let game_fut = async move {
            let sequence = async {
                self.lobby(&mut connections, &mut data, shutdown, &mut leaving)
                    .await;
                self.playing(&mut data, &mut connections).await;
            };

            select! {
                _ = sequence => {}
                _ = stop_rx.recv() => {}
            }

            (data, connections, leaving)
        };

        (stop_tx, game_fut)
    }
}

impl GameServer {
    async fn playing(&self, data: &mut GameData, connections: &mut Connections) {
        self.change_stage(Stage::Playing, connections, data);

        self.setup(connections, data).await;

        let mut rounds = 0;

        loop {
            connections.broadcast(Event::RoundStart(rounds));

            self.play_round(connections, data, rounds).await;

            connections.broadcast(Event::RoundEnd);
            connections.broadcast(Event::ConfirmNewRound);

            if !self.new_round(connections, data).await {
                connections.broadcast(Event::GameEnd);
                break;
            }

            rounds += 1;
        }
    }

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
                Winner::Player { uuid: winner.id() }
            } else {
                Winner::Tied
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
        connections.broadcast(Event::Setup);

        {
            let deck = &mut data.lock().deck;
            trace!("setting up deck");
            *deck = Deck::full();

            trace!("shuffling cards");
            let mut rng = rand::thread_rng();
            deck.shuffle(&mut rng);
        }

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
    async fn lobby(
        &self,
        connections: &mut Connections,
        data: &mut GameData,
        shutdown: mpsc::Sender<(uuid::Uuid, player::CloseReason)>,
        leaving: &mut mpsc::Receiver<uuid::Uuid>,
    ) {
        trace!("enter lobby");
        self.change_stage(Stage::Lobby, connections, data);

        let listener = TcpListener::bind((
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            self.config.server_port,
        ))
        .await
        .expect("failed to create server port");

        info!("listening on {:?}", listener.local_addr().ok());

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
                Some(id) = leaving.recv() => {
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

impl GameServer {
    fn change_stage(&self, stage: Stage, connections: &Connections, data: &mut GameData) {
        data.lock().stage = stage;
        connections.broadcast(Event::StageChange(stage));
    }
}

/// Gets the hosts uuid
fn host_id(data: &GameData) -> Option<uuid::Uuid> {
    data.lock().players().first().map(|p| p.id())
}
