extern crate rocket;

use mongodb::Database;
use rocket::{Build, Rocket};
use rocket::tokio::join;
use crate::{index, redirector, mount_v1, connect, DOMAINS_COLLECTION, AUTH_COLLECTION, Domain};
use crate::database::{Auth, manage_database};

async fn drop_if_needed<T>(db: &Database, name: &str) {
    let e = db.collection::<T>(name).drop(None).await;
    if let Err(e) = e {
        panic!("{:?}", e);
    }
}

async fn rocket_build() -> Rocket<Build> {
    // remove dev data for tests
    let db = connect().await;
    let drop_domains = drop_if_needed::<Domain>(&db, DOMAINS_COLLECTION);
    let drop_auths = drop_if_needed::<Auth>(&db, AUTH_COLLECTION);
    join!(drop_domains, drop_auths);
    // create data for tests
    manage_database().await;
    // build, mount and launch
    let rocket = rocket::build()
        .mount("/", routes![index])
        .mount("/r", routes![redirector]);
    let rocket = mount_v1(rocket);
    rocket
}

mod test {
    use rocket::local::asynchronous::Client;
    use rocket::http::{ContentType, Status};
    use serde_json::Value;
    use crate::{AUTH_COLLECTION, connect, doc, Domain, DOMAINS_COLLECTION};
    use crate::database::Auth;
    use crate::tests::rocket_build;

    const ADMIN: &'static str = r#"{"name": "admin", "password": "pass"}"#;

    macro_rules! assert_value {
        ($v:expr, $s:expr) => {
            let res_list: Value = serde_json::from_str(&$v.into_string().await.unwrap()).unwrap();
            let example_list: Value = serde_json::from_str($s).unwrap();
            assert_eq!(res_list, example_list);
        }
    }

    macro_rules! client {
        ($f:ident, $m:ident, $s:expr, $b:expr) => {
            $f.$m($s)
                .header(ContentType::JSON)
                .body($b)
                .dispatch()
                .await
        };
        ($f:ident, $m:ident, $s:expr) => {
            $f.$m($s)
                .header(ContentType::JSON)
                .body(ADMIN)
                .dispatch()
                .await
        }
    }

    #[rocket::async_test]
    async fn create_redirect_list_delete() {
        let client = Client::tracked(rocket_build().await).await.expect("valid rocket instance");
        let db = connect().await;
        ///////////////////
        // check create
        let res = client!(client, post, "/api/v1/redirect/create?name=test&domain=https://example.com");
        assert_eq!(res.status(), Status::Ok);
        assert_value!(res, r#"{"success":true,"response": "Created redirect to 'https://example.com' named 'test'."}"#);
        let auth = db.collection::<Auth>(AUTH_COLLECTION).find_one(doc! {"name":"admin"}, None).await.unwrap().unwrap();
        let domain = db.collection::<Domain>(DOMAINS_COLLECTION).find_one(doc! {"name":"test"}, None).await.unwrap().unwrap();
        ///////////////////
        // check redirect
        let res = client.get("/r/test").dispatch().await;
        assert_eq!(res.status(), Status::SeeOther);
        assert_eq!(res.headers().get_one("Location"), Some("https://example.com"));
        ///////////////////
        // check list
        let res = client!(client, get, "/api/v1/redirect");
        assert_eq!(res.status(), Status::Ok);
        let ex = format!(r#"{{"success":true,"response": [{{"_id":{{"$oid":"{}"}},"name":"test","domain":"https://example.com","owner":{{"$oid":"{}"}}}}]}}"#, domain._id, auth._id);
        assert_value!(res, &ex);
        ///////////////////
        // check delete
        let res = client!(client, delete, "/api/v1/redirect/delete?name=test");
        assert_eq!(res.status(), Status::Ok);
        assert_value!(res, r#"{"success":true,"response": "Deleted redirect named 'test'"}"#);
    }

    #[rocket::async_test]
    async fn create_edit_redirect_delete() {
        let client = Client::tracked(rocket_build().await).await.expect("valid rocket instance");
        ///////////////////
        // check create
        let res = client!(client, post, "/api/v1/redirect/create?name=test&domain=https://example.com");
        assert_eq!(res.status(), Status::Ok);
        assert_value!(res, r#"{"success":true,"response": "Created redirect to 'https://example.com' named 'test'."}"#);
        ///////////////////
        // check redirect
        // - no change
        let res = client.get("/r/test").dispatch().await;
        assert_eq!(res.status(), Status::SeeOther);
        assert_eq!(res.headers().get_one("Location"), Some("https://example.com"));
        ///////////////////
        // check edit
        // - domain change
        let res = client!(client, put, "/api/v1/redirect/edit?name=test&domain=https://example.pl");
        assert_eq!(res.status(), Status::Ok);
        assert_value!(res, r#"{"success":true,"response": "Edited redirect, permission 'https://example.com' -> 'https://example.pl'"}"#);
        ///////////////////
        // check redirect
        // - domain change
        let res = client.get("/r/test").dispatch().await;
        assert_eq!(res.status(), Status::SeeOther);
        assert_eq!(res.headers().get_one("Location"), Some("https://example.pl"));
        ///////////////////
        // check edit
        // - name change
        let res = client!(client, put, "/api/v1/redirect/edit?name=test&newname=example");
        assert_eq!(res.status(), Status::Ok);
        ///////////////////
        // check redirect
        // - name change
        let res = client.get("/r/example").dispatch().await;
        assert_eq!(res.status(), Status::SeeOther);
        assert_eq!(res.headers().get_one("Location"), Some("https://example.pl"));
        ///////////////////
        // check edit
        // - name and domain change
        let res = client!(client, put, "/api/v1/redirect/edit?name=example&newname=test2&domain=https://google.com");
        assert_eq!(res.status(), Status::Ok);
        ///////////////////
        // check redirect
        // - name and domain change
        let res = client.get("/r/test2").dispatch().await;
        assert_eq!(res.status(), Status::SeeOther);
        assert_eq!(res.headers().get_one("Location"), Some("https://google.com"));
        ///////////////////
        // check delete
        let res = client!(client, delete, "/api/v1/redirect/delete?name=test2");
        assert_eq!(res.status(), Status::Ok);
        assert_value!(res, r#"{"success":true,"response": "Deleted redirect named 'test2'"}"#);
    }

    // #[rocket::async_test]
    // async fn create_list_edit_list_delete() {
    //
    // }
    //
    // #[rocket::async_test]
    // async fn create_more_edit_list_delete() {
    //
    // }
}