#[cfg(test)] mod tests;

#[macro_use]
extern crate rocket;

use rocket::{Config, response::Redirect, futures::TryStreamExt};
use serde::Deserialize;
use reql::{r, cmd::connect::Options, Session};
use serde_json::{Value};

#[allow(dead_code)]
#[derive(Deserialize, Debug)]
struct Domain {
    id: u64,
    name: String,
    domain: String,
}

#[derive(Deserialize)]
struct ReConfig {
    db_host: String,
    db_port: u16,
    db_user: String,
    db_password: String,
}

impl Default for ReConfig {
    fn default() -> Self {
        Self {
            db_host: "localhost".to_string(),
            db_port: 28015u16,
            db_user: "admin".to_string(),
            db_password: "".to_string(),
        }
    }
}

const DOMAIN: &str = "https://lmpk.tk";
const DATABASE_NAME: &str = "redirector";
// table name for domains in debug
#[cfg(debug_assertions)]
const TABLE_NAME: &str = "domainsDev";
// table name for domains in release
#[cfg(not(debug_assertions))]
const TABLE_NAME: &str = "domains";

#[get("/<name>")]
async fn redirector(name: String) -> Redirect {
    let conn = match get_conn().await {
        Ok(conn) => conn,
        Err(_) => return Redirect::to(DOMAIN)
    };
    let mut query = r
        .db(DATABASE_NAME)
        .table(TABLE_NAME)
        .run::<_, Domain>(&conn);
    let mut route = DOMAIN.to_string();
    while let Ok(domain) = query.try_next().await {
        if let Some(domain) = domain {
            if name == domain.name {
                route = domain.domain;
                break;
            }
        } else {
            break;
        }
    };
    Redirect::to(route)
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/")]
fn check_list() -> &'static str {
    "Hello, world!"
}

#[post("/create?<name>&<domain>")]
fn create_redirect(name: Option<String>, domain: Option<String>) {}

#[put("/edit?<name>&<domain>")]
fn edit_redirect(name: Option<String>, domain: Option<String>) -> &'static str {
    "Hello, world!"
}

#[delete("/delete?<name>")]
fn remove_redirect(name: Option<String>) -> &'static str {
    "Hello, world!"
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    let conn = match get_conn().await {
        Ok(conn) => conn,
        Err(_) => panic!("Can't connect to the database")
    };
    // create database if needed
    let mut query = r
        .db_create(DATABASE_NAME)
        .run::<_, Value>(&conn);
    if query.try_next().await.is_ok() {
        println!("Database created");
    }
    // create table if needed
    let mut query = r
        .db(DATABASE_NAME)
        .table_create(TABLE_NAME)
        .run::<_, Value>(&conn);
    if query.try_next().await.is_ok() {
        println!("Table created");
    }

    // build, mount and launch
    let _rocket = rocket::build()
        .mount("/", routes![index])
        // change `r` to change redirecting prefix e.g. example.com/r/<name of redirect>
        .mount("/r", routes![redirector])
        .mount("/api/v1", routes![check_list, create_redirect, edit_redirect, remove_redirect])
        .launch()
        .await?;

    Ok(())
}

async fn get_conn() -> reql::Result<Session> {
    // get database configs
    let conf = match Config::figment().extract::<ReConfig>() {
        Ok(conf) => conf,
        Err(_) => {
            println!("Database config not found. Using default values");
            ReConfig::default()
        }
    };
    // connect to database
    let options = Options::new()
        .host(conf.db_host)
        .port(conf.db_port)
        .user(conf.db_user)
        .password(conf.db_password);
    let conn = r.connect(options).await;
    conn
}