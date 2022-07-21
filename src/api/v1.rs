use reql::r;
use rocket::{Build, Rocket};
use rocket::futures::TryStreamExt;
use serde::Serialize;
use rocket::serde::json::Json;
use serde_json::Value;
use crate::{DATABASE_NAME, Domain, get_conn, TABLE_NAME};

#[derive(Serialize)]
struct Response {
    success: bool,
    response: Value,
}

pub(crate) fn mount_v1(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount(
        "/api/v1/redirect",
        routes![
            check_domains,
            create_redirect,
            edit_redirect,
            remove_redirect,
    //     TODO: create_random_redirect - everyone will be able to use this (if I don't find easy way to ratelimit then it will be limited)
            ],
    )
    // .mount("/api/v1/auth",
    //        routes![
    //     TODO: add_auth, edit_auth, delete_auth,
    //  ],
    // )
}

#[get("/")]
async fn check_domains() -> Json<Response> {
    let conn = match get_conn().await {
        Ok(conn) => conn,
        Err(_) => return Json(Response {
            success: false,
            response: Value::String("Server error. Can't connect to the database. Contact the developer".to_string()),
        })
    };
    let mut query = r
        .db(DATABASE_NAME)
        .table(TABLE_NAME)
        .run::<_, Domain>(&conn);
    let mut domains: Vec<Value> = vec![];
    while let Ok(domain) = query.try_next().await {
        if let Some(domain) = domain {
            domains.push(serde_json::to_value(domain).unwrap());
        } else {
            break;
        }
    };
    Json(Response {
        success: true,
        response: Value::Array(domains),
    })
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