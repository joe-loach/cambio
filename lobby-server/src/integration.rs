use super::*;

use axum::http::StatusCode;
use axum_test::TestServer;
use serde_json::json;
use tempdir::TempDir;

fn setup(temp_dir: &TempDir) -> anyhow::Result<Router> {
    log::register();

    dotenvy::dotenv().expect("failed to load config");

    let db = db::establish_connection().expect("failed to connect to database");
    let db = db.with_inner(|db| db.snapshot(&db::MODELS, &temp_dir.path().join("temp.db")))?;
    let db = db::Db::from_inner(db);

    let state = Arc::new(AppState { db });

    Ok(router(state))
}

#[tokio::test]
async fn register_create_game_and_list_then_join() -> anyhow::Result<()> {
    let temp_dir = TempDir::new("lobby-server-integration")?;

    let app = setup(&temp_dir)?;
    // Create the test server
    // need to add connect info otherwise the rate limiting won't work
    let server = TestServer::new(app.into_make_service_with_connect_info::<SocketAddr>())?;

    let username = "username";
    let password = "password";

    let details = json!({
        "username": username,
        "password": password,
    });

    // register and login
    let register = server.post("/register").json(&details).await;
    register.assert_status(StatusCode::OK);

    let login = server.get("/login").json(&details).await;
    login.assert_status(StatusCode::OK);
    let response = login.json::<routes::login::LoginResponse>();

    // extract the token
    let token = response.access_token;
    println!("{}", token);

    // create a new game
    let game_name = "my_game";
    let create = server
        .post("/create")
        // use our token here to auth ourselves
        .authorization_bearer(&token)
        .json(&json! ({
            "name": game_name,
            // a public game so we can see it in the list
            "visibility": "public",
            "server_addr": "127.0.0.1:9705",
        }))
        .await;
    create.assert_status(StatusCode::OK);
    let create = create.json::<routes::create::CreateGameResponse>();
    // the Id of the game we've just created
    let created_game_id = create.id;

    // list the public games
    let list = server.get("/list").await;
    list.assert_status(StatusCode::OK);
    // it should contain our newly made game
    let game_list = list.json::<routes::list::GameListResponse>();
    let game_exists = game_list
        .game_info
        .iter()
        .any(|info| info.name == game_name);
    assert!(game_exists, "Game we've just created should be listed");

    // "join" the game using the Id
    let join = server
        .get(&format!("/join/{}", created_game_id.as_str()))
        // use our token here to auth ourselves
        .authorization_bearer(&token)
        .await;
    join.assert_status(StatusCode::OK);
    let join = join.json::<routes::join::JoinGameResponse>();
    // we have the address!
    let _ = join.server_addr;

    temp_dir.close()?;
    Ok(())
}
