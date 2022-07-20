use rocket::{Build, Rocket};
use serde::Serialize;
use rocket::serde::json::Json;
use crate::get_conn;

#[derive(Serialize)]
struct Response {
    success: bool,
    response: String,
}

pub(crate) fn mount_v1(rocket: Rocket<Build>) -> Rocket<Build> {
    rocket.mount(
        "/api/v1",
        routes![
            check_list,
            create_redirect,
            edit_redirect,
            remove_redirect,
        ],
    )
}

#[get("/")]
async fn check_list() -> Json<Response> {
    let conn = get_conn().await;
    if conn.is_err() {
        return Json(Response {
            success: false,
            response: "Server error. Can't connect to the database. Contact the developer".to_string(),
        });
    }
    Json(Response {
        success: true,
        response: "".to_string(),
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