#[macro_use]
extern crate rocket;

use std::borrow::Cow;
use dotenv::var;
use rocket::{Build, Rocket, response::Redirect};
use serde::Deserialize;
use reql::{r, cmd::connect::Options};
use rocket::futures::TryStreamExt;
use serde_json::Value;

#[derive(Deserialize, Debug)]
struct Domain {
    id: u64,
    name: String,
    domain: String,
}

const DOMAIN: &str = "https://lmpk.tk";

#[get("/<name>")]
async fn redirector(name: String) -> Redirect {
    let options = Options::new()
        .host(var("DB_HOST").unwrap())
        .port(var("DB_PORT").unwrap().parse::<u16>().unwrap())
        .db(var("DB_DB").unwrap())
        .user(var("DB_USER").unwrap())
        .password(var("DB_PASSWORD").unwrap());
    let conn = r.connect(options).await;
    if conn.is_err() {
        return Redirect::to(DOMAIN);
    }
    let conn = conn.unwrap().clone();
    let mut query = r.db("redirector").table("domains").run(&conn);
    let mut route = DOMAIN.to_string();
    while let Ok(domain) = query.try_next().await {
        if let Some(domain) = domain {
            let domain: Domain = serde_json::from_value(domain).unwrap();
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
    dotenv::dotenv().ok();
    let _rocket = rocket::build()
        .mount("/", routes![index])
        // change `r` to change redirecting prefix e.g. example.com/r/<name of redirect>
        .mount("/r", routes![redirector])
        .mount("/api/v1", routes![check_list, create_redirect, edit_redirect, remove_redirect])
        .launch()
        .await?;

    Ok(())
}
