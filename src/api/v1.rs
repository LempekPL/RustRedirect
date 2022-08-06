use mongodb::bson::doc;
use rocket::{Build, Request, request, Rocket};
use rocket::futures::TryStreamExt;
use rocket::http::Status;
use rocket::outcome::Outcome;
use rocket::request::FromRequest;
use serde::Serialize;
use rocket::serde::json::Json;
use serde_json::Value;
use crate::{AUTH_COLLECTION, connect, Domain, DOMAINS_COLLECTION};
use crate::database::Auth;

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
            i_create_post,
            i_edit_put,
            i_delete_delete,
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
fn create_redirect(name: Option<String>, domain: Option<String>, token: Token) -> Json<Response> {

    Json(Response {
        success: true,
        response: Value::String(format!("Created redirect to '{}' named '{}'. Made by using token named: {}", domain.unwrap(), name.unwrap(), token.0)),
    })
}

#[put("/edit?<name>&<domain>")]
fn edit_redirect(name: Option<String>, domain: Option<String>) -> &'static str {
    "Hello, world!"
}

#[delete("/delete?<name>")]
fn remove_redirect(name: Option<String>) -> &'static str {
    "Hello, world!"
}


// info paths
#[get("/create")]
fn i_create_post() -> Json<Response> {
    Json(Response {
        success: false,
        response: Value::String("Use post".to_string()),
    })
}
#[get("/edit")]
fn i_edit_put() -> Json<Response> {
    Json(Response {
        success: false,
        response: Value::String("Use put".to_string()),
    })
}
#[get("/delete")]
fn i_delete_delete() -> Json<Response> {
    Json(Response {
        success: false,
        response: Value::String("Use delete".to_string()),
    })
}



struct Token(String, String);

#[derive(Debug)]
enum TokenError {
    Missing,
    Invalid,
    Unknown
}

#[async_trait]
impl<'a> FromRequest<'a> for Token {
    type Error = TokenError;

    async fn from_request(request: &'a Request<'_>) -> request::Outcome<Self, Self::Error> {
        let token = request.headers().get_one("token");
        match token {
            Some(token) => {
                let col = connect().await.collection::<Auth>(AUTH_COLLECTION);
                let found = match col.find_one(doc! {"token": token}, None).await {
                    Ok(f) => f,
                    Err(e) => {
                        println!("Error whilst getting the token: {:?}", *e.kind);
                        return Outcome::Failure((Status::Unauthorized, TokenError::Unknown))
                    }
                };
                let name = match found {
                    None => return Outcome::Failure((Status::Unauthorized, TokenError::Invalid)),
                    Some(t) => t.name,
                };
                Outcome::Success(Token(name))
            },
            None => Outcome::Failure((Status::Unauthorized, TokenError::Missing))
        }
    }
}