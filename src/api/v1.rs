use rocket::{Build, Rocket};
use rocket::futures::TryStreamExt;
use serde::Serialize;
use rocket::serde::json::Json;
use serde_json::{Error, Value};
use crate::{connect, DATABASE_NAME, Domain, DOMAINS_COLLECTION};

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
    let col = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
    let cursor = match col.find(None, None).await {
        Ok(c) => c,
        Err(e) => {
            println!("[38 line v1.rs] {:?}", *e.kind);
            return Json(Response {
                success: false,
                response: Value::String("Could not process (server error)".to_string()),
            });
        },
    };
    let cursor: Vec<Domain> = match cursor.try_collect().await {
        Ok(c) => c,
        Err(e) => {
            println!("[48 line v1.rs] {:?}", *e.kind);
            return Json(Response {
                success: false,
                response: Value::String("Could not process (server error)".to_string()),
            });
        },
    };
    let cursor = match serde_json::to_value(cursor) {
        Ok(c) => c,
        Err(e) => {
            println!("[58 line v1.rs] {:?}", e.to_string());
            return Json(Response {
                success: false,
                response: Value::String("Could not process (server error)".to_string()),
            });
        },
    };


    Json(Response {
        success: true,
        response: cursor,
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