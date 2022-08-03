#[allow(non_snake_case)]
#[cfg(test)]
mod tests;
mod api;
mod database;

#[macro_use]
extern crate rocket;

use rocket::response::Redirect;
use serde_json::Value;
use crate::api::v1::mount_v1;
use crate::database::{Conn, Domain, filter_name};

const DOMAIN: &str = "https://lmpk.tk";
const DATABASE_NAME: &str = "redirector";
// table name for domains in debug
#[cfg(debug_assertions)]
const TABLE_NAME: &str = "domainsDev";
// table name for domains in release
#[cfg(not(debug_assertions))]
const TABLE_NAME: &str = "domains";
// table name for auth codes
const AUTH_TABLE_NAME: &str = "auth";

#[get("/<name>")]
async fn redirector(name: String) -> Redirect {
    let conn = match Conn::new().await {
        Ok(c) => c,
        Err(e) => {
            println!("{}", e);
            return Redirect::to(DOMAIN);
        }
    };
    // let route_value = serde_json::from_str(&format!(r#"{{"name":"{}"}}"#, name)).unwrap();
    // let route_value = |a: Domain| a.name == name;
    let route = conn.get_filtered_for_domain(DATABASE_NAME, TABLE_NAME, name).await;
    if route.len() == 1 {
        let r = match route.get(0) {
            None => DOMAIN.to_string(),
            Some(d) => d.clone().domain
        };
        Redirect::to(r)
    } else {
        Redirect::to(DOMAIN)
    }
}

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    database::manage_database().await;
    // build, mount and launch
    let rocket = rocket::build()
        .mount("/", routes![index])
        // change `r` to change redirecting prefix e.g. example.com/r/<name of redirect>
        .mount("/r", routes![redirector]);
    let rocket = mount_v1(rocket);
    let _rocket = rocket.launch()
        .await?;

    Ok(())
}