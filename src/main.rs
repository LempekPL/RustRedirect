#[allow(non_snake_case)]
#[cfg(test)]
mod tests;
mod api;
mod database;

#[macro_use]
extern crate rocket;

use mongodb::bson::doc;
use rocket::response::Redirect;
use crate::api::v1::mount_v1;
use crate::database::{connect, Domain};

const DOMAIN: &str = "https://lmpk.tk";
const DATABASE_NAME: &str = "redirector";
// collection for domains in debug
#[cfg(debug_assertions)]
const DOMAINS_COLLECTION: &str = "domainsDev";
// collection for domains in release
#[cfg(not(debug_assertions))]
const DOMAINS_COLLECTION: &str = "domains";
// collection for auth codes
const AUTH_COLLECTION: &str = "auth";

#[get("/<name>")]
async fn redirector(name: String) -> Redirect {
    let col = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
    let filter = doc! { "name" : name };
    let dom = match col.find_one(filter, None).await {
        Ok(d) => d,
        Err(_) => return Redirect::to(DOMAIN)
    };
    match dom {
        Some(d) => Redirect::to(d.domain),
        None => Redirect::to(DOMAIN)
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

macro_rules! reowrap {
    ( $e:expr, $r:expr ) => {
        match $e {
            Some(x) => x,
            None => return $r,
        }
    }
}

macro_rules! rerwrap {
    ( $e:expr, $r:expr ) => {
        match $e {
            Ok(x) => x,
            Err(_) => return $r,
        }
    }
}

pub(crate) use reowrap;
pub(crate) use rerwrap;