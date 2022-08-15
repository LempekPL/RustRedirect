use mongodb::{
    Collection,
    bson::{doc, Document},
};
use rand::{
    SeedableRng,
    distributions::{Alphanumeric, DistString},
};
use regex::Regex;
use rocket::{
    Build,
    Rocket,
    futures::TryStreamExt,
    serde::json::Json
};
use serde::{Serialize, Deserialize};
use serde_json::Value;
use crate::{AUTH_COLLECTION, DOMAINS_COLLECTION, Domain, connect, some_return, ok_return, add_and};
use crate::database::{Auth, Permission};

#[derive(Serialize)]
struct Response {
    success: bool,
    response: Value,
}

#[derive(Deserialize, Clone)]
struct PreAuth {
    name: String,
    password: String,
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
    let rocket = rocket.mount(
        "/api/v1/auth",
        routes![
            list_auth,
            create_auth,
            edit_auth,
            delete_auth,
            i_create_post,
            i_edit_put,
            i_delete_delete,
        ],
    );
    rocket
}

////////////
// DOMAINS
////////////

#[get("/", data = "<user>")]
async fn check_domains(user: Json<PreAuth>) -> Json<Response> {
    let auth = match authorize(user).await {
        Ok(a) => a,
        Err(e) => return e,
    };
    let document;
    if auth.permission.can_list() {
        document = None;
    } else if auth.permission.can_own() {
        document = Some(doc! { "owner": auth._id });
    } else {
        return Response::PERMISSIONS_TOO_LOW().json();
    }
    let conn = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
    let cursor = ok_return!(conn.find(document, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
    let collected: Vec<Domain> = ok_return!(cursor.try_collect().await, Response::DATABASE_WHILST_TRYING_TO_COLLECT().json());
    let collected = ok_return!(serde_json::to_value(collected), Response::SERVER_WHILST_TRYING_TO_FORMAT().json());
    Response {
        success: true,
        response: collected,
    }.json()
}

#[post("/random?<domain>", data = "<user>")]
async fn random_redirect(domain: Option<String>, user: Json<PreAuth>) -> Json<Response> {
    let auth = match authorize(user).await {
        Ok(a) => a,
        Err(e) => return e,
    };
    let domain = some_return!(domain, Response::USER_DID_NOT_PROVIDE_PARAM("domain").json());
    let domain_regex = Regex::new(r#"https?://[^-][A-z\d-]{1,63}(?:\.[^-][A-z\d-]+){0,63}\.[A-z]{2,}"#).unwrap();
    if domain_regex.is_match(&domain) {
        return Response::NOT_ALLOWED_DOMAIN_FORMAT().json();
    }
    let db = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
    if auth.permission.can_random() {
        let name = match get_check_random(&db, Alphanumeric.sample_string(&mut rand::rngs::SmallRng::from_entropy(), 8), 3).await {
            Ok(o) => o,
            Err(e) => return e
        };
        let res = db.insert_one(
            Domain {
                _id: Default::default(),
                name: name.clone(),
                domain: domain.clone(),
                owner: auth._id,
            }, None).await;
        return match res {
            Ok(_) => Response::new(true, &format!("Created random redirect to '{}' named '{}'.", domain, name)).json(),
            Err(_) => Response::COULD_NOT("create", "random redirect").json()
        };
    } else {
        Response::PERMISSIONS_TOO_LOW().json()
    }
}

#[async_recursion::async_recursion]
async fn get_check_random(db: &Collection<Domain>, name: String, tries: u32) -> Result<String, Json<Response>> {
    if tries == 0 {
        return Err(Response::COULD_NOT("create", "random redirect").json());
    }
    let dom = ok_return!(db.find_one(doc! { "name": name.clone() }, None).await, Err(Response::DATABASE_WHILST_TRYING_TO_FIND().json()));
    return match dom {
        Some(_) => get_check_random(db, Alphanumeric.sample_string(&mut rand::rngs::SmallRng::from_entropy(), 8), tries - 1).await,
        None => Ok(name)
    };
}

#[post("/create?<name>&<domain>", data = "<user>")]
async fn create_redirect(name: Option<String>, domain: Option<String>, user: Json<PreAuth>) -> Json<Response> {
    let auth = match authorize(user).await {
        Ok(a) => a,
        Err(e) => return e,
    };
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let domain = some_return!(domain, Response::USER_DID_NOT_PROVIDE_PARAM("domain").json());
    let domain_regex = Regex::new(r#"https?://[^-][A-z\d-]{1,63}(?:\.[^-][A-z\d-]+){0,63}\.[A-z]{2,}"#).unwrap();
    if domain_regex.is_match(&domain) {
        return Response::NOT_ALLOWED_DOMAIN_FORMAT().json();
    }
    if auth.permission.can_own() {
        let db = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
        let dom: Option<Domain> = ok_return!(db.find_one(doc! { "name" : name.clone() }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
        if dom.is_some() {
            return Response::EXIST("Redirect", "already").json();
        }
        let res = db.insert_one(
            Domain {
                _id: Default::default(),
                name: name.clone(),
                domain: domain.clone(),
                owner: auth._id,
            }, None).await;
        return match res {
            Ok(_) => Response::new(true, &format!("Created redirect to '{}' named '{}'.", domain, name)).json(),
            Err(_) => Response::COULD_NOT("create", "redirect").json()
        };
    } else {
        Response::PERMISSIONS_TOO_LOW().json()
    }
}

#[put("/edit?<name>&<newname>&<domain>", data = "<user>")]
async fn edit_redirect(name: Option<String>, newname: Option<String>, domain: Option<String>, user: Json<PreAuth>) -> Json<Response> {
    let auth = match authorize(user).await {
        Ok(a) => a,
        Err(e) => return e,
    };
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let domain_regex = Regex::new(r#"https?://[^-][A-z\d-]{1,63}(?:\.[^-][A-z\d-]+){0,63}\.[A-z]{2,}"#).unwrap();
    if domain.is_some() && domain_regex.is_match(&domain.clone().unwrap()) {
        return Response::NOT_ALLOWED_DOMAIN_FORMAT().json();
    }
    let search_name = match get_search(auth, &name) {
        Ok(o) => o,
        Err(e) => return e
    };
    let db = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
    if let Some(newname) = newname.clone() {
        let existing_domain: Option<Domain> = ok_return!(db.find_one(doc! { "name" : newname }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
        if existing_domain.is_some() {
            return Response::EXIST("Domain with the new name", "already").json();
        }
    }
    let dom: Option<Domain> = ok_return!(db.find_one(search_name, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
    let dom = match dom {
        None => return Response::EXIST("Redirect", "doesn't").json(),
        Some(d) => d
    };
    let res = db
        .update_one(
            doc! { "_id" : dom._id },
            doc! { "$set": { "name": newname.clone().unwrap_or(name.clone()), "domain": domain.clone().unwrap_or(dom.domain.clone()) } },
            None)
        .await;
    match res {
        Ok(m) if m.modified_count > 0 => {
            if newname.is_none() && domain.is_none() {
                return Response::NOTHING_CHANGED().json();
            }
            let mut str = "".to_string();
            if let Some(newname) = newname {
                str += &format!("name '{}' -> '{}'", name, newname);
            }
            if let Some(domain) = domain {
                add_and!(str);
                str += &format!("permission '{}' -> '{}'", dom.domain, domain);
            }
            return Response::new(true, &format!("Edited redirect, {}", str)).json();
        }
        Ok(_) => Response::NOTHING_CHANGED().json(),
        Err(_) => Response::COULD_NOT("edit", "redirect").json()
    }
}

#[delete("/delete?<name>", data = "<user>")]
async fn remove_redirect(name: Option<String>, user: Json<PreAuth>) -> Json<Response> {
    let auth = match authorize(user).await {
        Ok(a) => a,
        Err(e) => return e,
    };
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let search_name = match get_search(auth, &name) {
        Ok(o) => o,
        Err(e) => return e
    };
    let db = connect().await.collection::<Domain>(DOMAINS_COLLECTION);
    let dom: Option<Domain> = ok_return!(db.find_one(search_name, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
    match dom {
        Some(dom) => {
            let res = db.delete_one(doc! { "_id": dom._id }, None).await;
            match res {
                Ok(r) if r.deleted_count > 0 => Response::new(true, &format!("Deleted redirect named '{}'", name)).json(),
                Ok(_) => Response::NOTHING_DELETED().json(),
                Err(_) => Response::COULD_NOT("delete", "redirect").json()
            }
        }
        None => Response::COULD_NOT("find", "redirect").json()
    }
}

////////////
// AUTHS
////////////

#[get("/", data = "<user>")]
async fn list_auth(user: Json<PreAuth>) -> Json<Response> {
    let auth = match authorize(user).await {
        Ok(a) => a,
        Err(e) => return e,
    };
    let document;
    if auth.permission.can_admin() {
        document = None
    } else if auth.permission.can_manage() {
        document = Some(doc! { "permission.0": {"$ne": 1}})
    } else {
        return Response::PERMISSIONS_TOO_LOW().json();
    }
    let conn = connect().await.collection::<Auth>(AUTH_COLLECTION);
    let cursor = ok_return!(conn.find(document, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
    let collected: Vec<Auth> = ok_return!(cursor.try_collect().await, Response::DATABASE_WHILST_TRYING_TO_COLLECT().json());
    let collected = ok_return!(serde_json::to_value(collected), Response::SERVER_WHILST_TRYING_TO_FORMAT().json());
    Response {
        success: true,
        response: collected,
    }.json()
}

#[post("/create?<name>&<password>&<permission>", data = "<user>")]
async fn create_auth(name: Option<String>, password: Option<String>, permission: Option<u8>, user: Json<PreAuth>) -> Json<Response> {
    let auth = match authorize(user).await {
        Ok(a) => a,
        Err(e) => return e,
    };
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let password = some_return!(password, Response::USER_DID_NOT_PROVIDE_PARAM("password").json());
    let permission = match permission {
        None => Permission::default(),
        Some(p) => Permission::from_u8(p)
    };
    return if auth.permission.can_admin() || (auth.permission.can_manage() && !permission.can_manage()) {
        let db = connect().await.collection::<Auth>(AUTH_COLLECTION);
        let existing_auth: Option<Auth> = ok_return!(db.find_one(doc! { "name" : name.clone() }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
        if existing_auth.is_some() {
            return Response::EXIST("Auth with that name", "already").json();
        }
        let hashed = ok_return!(bcrypt::hash(password, bcrypt::DEFAULT_COST), Response::COULD_NOT("encrypt", "password").json());
        let res = db.insert_one(
            Auth {
                _id: Default::default(),
                name: name.clone(),
                password: hashed,
                permission,
            }, None).await;
        match res {
            Ok(_) => Response::new(true, &format!("Created auth named '{}' with permission: {}.", name, permission)).json(),
            Err(_) => Response::COULD_NOT("create", "auth").json()
        }
    } else {
        Response::PERMISSIONS_TOO_LOW().json()
    };
}

#[put("/edit?<name>&<newname>&<password>&<permission>", data = "<user>")]
async fn edit_auth(name: Option<String>, newname: Option<String>, password: Option<String>, permission: Option<u8>, user: Json<PreAuth>) -> Json<Response> {
    let auth = match authorize(user).await {
        Ok(a) => a,
        Err(e) => return e,
    };
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let permission = match permission {
        None => None,
        Some(p) => Some(Permission::from_u8(p))
    };

    return if auth.permission.can_admin() || (auth.permission.can_manage() && (permission.is_none() || !permission.clone().unwrap().can_manage())) {
        let db = connect().await.collection::<Auth>(AUTH_COLLECTION);
        if let Some(newname) = newname.clone() {
            let existing_auth: Option<Auth> = ok_return!(db.find_one(doc! { "name" : newname }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
            if existing_auth.is_some() {
                return Response::EXIST("Auth with the new name", "already").json();
            }
        }
        let old_auth: Option<Auth> = ok_return!(db.find_one(doc! { "name" : name.clone() }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
        let old_auth: Auth = some_return!(old_auth, Response::EXIST("Auth", "doesn't").json());
        if !auth.permission.can_admin() && old_auth.permission.can_manage() {
            return Response::PERMISSIONS_TOO_LOW().json();
        }
        let hashed;
        match password.clone() {
            None => hashed = None,
            Some(p) => hashed = Some(ok_return!(bcrypt::hash(p, bcrypt::DEFAULT_COST), Response::COULD_NOT("encrypt", "password").json()))
        }
        let res = db
            .update_one(
                doc! { "_id" : old_auth._id },
                doc! {
                    "$set": {
                        "name": newname.clone().unwrap_or(old_auth.name.clone()),
                        "password": hashed.clone().unwrap_or(old_auth.password.clone()),
                        "permission": permission.unwrap_or(old_auth.permission)
                    }
                },
                None)
            .await;
        match res {
            Ok(m) if m.modified_count > 0 => {
                if newname.is_none() && password.is_none() && permission.is_none() {
                    return Response::NOTHING_CHANGED().json();
                }
                let mut str = "".to_string();
                if let Some(newname) = newname {
                    str += &format!("name '{}' -> '{}'", old_auth.name, newname);
                }
                if password.is_some() {
                    add_and!(str);
                    str += &format!("password changed");
                }
                if let Some(permission) = permission {
                    add_and!(str);
                    str += &format!("permission '{}' -> '{}'", old_auth.permission, permission);
                }
                return Response::new(true, &format!("Edited auth, {}", str)).json();
            }
            Ok(_) => Response::NOTHING_CHANGED().json(),
            Err(_) => Response::COULD_NOT("edit", "redirect").json()
        }
    } else {
        Response::PERMISSIONS_TOO_LOW().json()
    };
}

#[delete("/delete?<name>", data = "<user>")]
async fn delete_auth(name: Option<String>, user: Json<PreAuth>) -> Json<Response> {
    let auth = match authorize(user).await {
        Ok(a) => a,
        Err(e) => return e,
    };
    let name = some_return!(name, Response::USER_DID_NOT_PROVIDE_PARAM("name").json());
    let db = connect().await.collection::<Auth>(AUTH_COLLECTION);
    let del_auth = ok_return!(db.find_one(doc! { "name": name.clone() }, None).await, Response::DATABASE_WHILST_TRYING_TO_FIND().json());
    match del_auth {
        Some(del_auth) => {
            if auth.permission.can_admin() || (auth.permission.can_manage() && !del_auth.permission.can_manage()) {
                let res = db.delete_one(doc! { "_id": del_auth._id }, None).await;
                match res {
                    Ok(r) if r.deleted_count > 0 => Response::new(true, &format!("Deleted auth named '{}'", name)).json(),
                    Ok(_) => Response::NOTHING_DELETED().json(),
                    Err(_) => Response::COULD_NOT("delete", "auth").json()
                }
            } else {
                return Response::PERMISSIONS_TOO_LOW().json();
            }
        }
        None => Response::COULD_NOT("find", "redirect").json()
    }
}

//////////
// AUTH
//////////

async fn authorize(user: Json<PreAuth>) -> Result<Auth, Json<Response>> {
    let col = connect().await.collection::<Auth>(AUTH_COLLECTION);
    let found = ok_return!(col.find_one(doc! {"name": user.name.clone()}, None).await, Err(Response::DATABASE_WHILST_TRYING_TO_FIND().json()));
    let auth = some_return!(found, Err(Response::USER_NOT_FOUND().json()));
    let ver = ok_return!(bcrypt::verify(user.password.clone(), &auth.password), Err(Response::BCRYPT_WHILST_TRYING_TO_VERIFY().json()));
    return if ver {
        Ok(auth)
    } else {
        Err(Response::WRONG_PASSWORD().json())
    };
}

//////////////
// RESPONSES
//////////////

impl Response {
    fn new(success: bool, response: &str) -> Self {
        Self {
            success,
            response: Value::String(response.to_string()),
        }
    }

    fn json(self) -> Json<Self> {
        Json(self)
    }

    const DATABASE_WHILST_TRYING_TO_FIND: fn() -> Response = || Response::new(false, "Database error whilst trying to find.");
    const DATABASE_WHILST_TRYING_TO_COLLECT: fn() -> Response = || Response::new(false, "Database error whilst trying to collect data.");
    const SERVER_WHILST_TRYING_TO_FORMAT: fn() -> Response = || Response::new(false, "Server error whilst response formatting.");
    const USER_DID_NOT_PROVIDE_PARAM: fn(&str) -> Response = |param: &str| Response::new(false, &format!("User error, did not provide '{}' param.", param));
    const PERMISSIONS_TOO_LOW: fn() -> Response = || Response::new(false, "Could not do that. Permissions too low.");
    const EXIST: fn(&str, &str) -> Response = |thing: &str, action: &str| Response::new(false, &format!("{} {} exist.", thing, action));
    const COULD_NOT: fn(&str, &str) -> Response = |action: &str, thing: &str| Response::new(false, &format!("Could not {} {}.", action, thing));
    const NOTHING_CHANGED: fn() -> Response = || Response::new(false, "Nothing changed.");
    const NOTHING_DELETED: fn() -> Response = || Response::new(false, "Nothing deleted.");
    const NOT_ALLOWED_DOMAIN_FORMAT: fn() -> Response = || Response::new(false, "Sent domain doesn't match the format e. g. https://example.com.");

    const USER_NOT_FOUND: fn() -> Response = || Response::new(false, "User not found.");
    const WRONG_PASSWORD: fn() -> Response = || Response::new(false, "Wrong password.");
    const BCRYPT_WHILST_TRYING_TO_VERIFY: fn() -> Response = || Response::new(false, "Bcrypt error whilst trying to verify user.");
}

//////////////////
// INFO PATHS
//////////////////

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

//////////
// OTHER
//////////

fn get_search(auth: Auth, name: &str) -> Result<Document, Json<Response>> {
    if auth.permission.can_mod() {
        Ok(doc! { "name": name.clone() })
    } else if auth.permission.can_own() {
        Ok(doc! { "name": name.clone(), "owner": auth._id })
    } else {
        Err(Response::PERMISSIONS_TOO_LOW().json())
    }
}