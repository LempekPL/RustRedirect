use std::fmt::Debug;
use bcrypt::{BcryptResult, verify};
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
    let rocket = rocket.mount(
        "/api/v1/redirect",
        routes![
            check_domains,
            create_redirect,
            edit_redirect,
            remove_redirect,
            random_redirect,
            i_create_post,
            i_edit_put,
            i_delete_delete,
            i_random_post,
            ],
    );
    // .mount("/api/v1/auth",
    //        routes![
    //     TODO: add_auth, edit_auth, delete_auth,
    //  ],
    // )
    rocket
}

#[get("/")]
async fn check_domains() -> Json<Response> {
    let col = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
    let cursor = match col.find(None, None).await {
        Ok(c) => c,
        Err(e) => {
            println!("finding db err: {:?}", *e.kind);
            return Json(Response {
                success: false,
                response: Value::String("Database error. Try again".to_string()),
            });
        }
    };
    let cursor: Vec<Domain> = match cursor.try_collect().await {
        Ok(c) => c,
        Err(e) => {
            println!("cursor collecting db err: {:?}", *e.kind);
            return Json(Response {
                success: false,
                response: Value::String("Database error. Try again".to_string()),
            });
        }
    };
    let cursor = match serde_json::to_value(cursor) {
        Ok(c) => c,
        Err(e) => {
            println!("to value err: {:?}", e.to_string());
            return Json(Response {
                success: false,
                response: Value::String("Response formatting. Try again".to_string()),
            });
        }
    };
    Json(Response {
        success: true,
        response: cursor,
    })
}

#[post("/random?<domain>")]
fn random_redirect(domain: Option<String>, auth: Auth) -> Json<Response> {
    // if auth.permission.can_random() {
    //
    // }
    Json(Response {
        success: true,
        response: Value::String(format!("Created redirect to '' named ''. Made by using token named: ")),
    })
}

#[post("/create?<name>&<domain>")]
async fn create_redirect(name: Option<String>, domain: Option<String>, auth: Auth) -> Json<Response> {
    let name = match name {
        None => return Json(Response {
            success: false,
            response: Value::String("Did not provide 'name' param".to_string()),
        }),
        Some(name) => name
    };
    let domain = match domain {
        None => return Json(Response {
            success: false,
            response: Value::String("Did not provide 'domain' param".to_string()),
        }),
        Some(domain) => domain
    };
    if auth.permission.can_own() {
        let db = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
        let dom = match db.find_one(doc! { "name" : name.clone() }, None).await {
            Ok(dom) => dom,
            Err(e) => {
                println!("can own db err: {:?}", *e.kind);
                return Json(Response {
                    success: false,
                    response: Value::String("Database error. Try again".to_string()),
                });
            }
        };
        return match dom {
            None => {
                let res = db.insert_one(Domain {
                    name: name.clone(),
                    domain: domain.clone(),
                }, None).await;
                return match res {
                    Ok(_) => Json(Response {
                        success: true,
                        response: Value::String(format!("Created redirect to '{}' named '{}'. Made by using token named: {}", domain, name, auth.name)),
                    }),
                    Err(e) => {
                        println!("can own db err: {:?}", *e.kind);
                        Json(Response {
                            success: false,
                            response: Value::String("Could not create redirect.".to_string()),
                        })
                    }
                };
            }
            Some(_) => {
                Json(Response {
                    success: false,
                    response: Value::String("Redirect with that name already exists".to_string()),
                })
            }
        };
    } else {
        Json(Response {
            success: false,
            response: Value::String("Could not create redirect. Permissions too low.".to_string()),
        })
    }
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

#[get("/random")]
fn i_random_post() -> Json<Response> {
    Json(Response {
        success: false,
        response: Value::String("Use post".to_string()),
    })
}

#[derive(Debug)]
pub(crate) enum AuthError {
    MissingToken,
    MissingName,
    Invalid,
    ServerError,
    NotFound,
    Unknown,
}

#[rocket::async_trait]
impl<'a> FromRequest<'a> for Auth {
    type Error = AuthError;

    async fn from_request(request: &'a Request<'_>) -> request::Outcome<Self, Self::Error> {
        let name = match request.headers().get_one("name") {
            None => return Outcome::Failure((Status::BadRequest, AuthError::MissingName)),
            Some(n) => n
        };
        // connect to the database and select auth collection
        let col = connect().await.collection::<Auth>(AUTH_COLLECTION);
        // check for auth
        let found = match col.find_one(doc! {"name": name}, None).await {
            Ok(f) => f,
            Err(e) => {
                println!("Error while getting data: {:?}", *e.kind);
                return Outcome::Failure((Status::InternalServerError, AuthError::ServerError));
            }
        };
        let auth = match found {
            None => return Outcome::Failure((Status::Unauthorized, AuthError::NotFound)),
            Some(t) => Auth { name: t.name, token: t.token, permission: t.permission },
        };
        let token = request.headers().get_one("token");
        let auth = match token {
            None => return Outcome::Failure((Status::Unauthorized, AuthError::MissingToken)),
            Some(token)  => {
                let ver = verify(token, &auth.token);
                match ver {
                    Ok(ok) => {
                        if ok {
                            auth
                        } else {
                            return Outcome::Failure((Status::Unauthorized, AuthError::Invalid))
                        }
                    }
                    Err(_) => return Outcome::Failure((Status::Unauthorized, AuthError::Unknown))
                }
            }
        };
        Outcome::Success(auth)
    }
}