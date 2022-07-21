extern crate rocket;

use reql::r;
use rocket::{Build, Rocket, futures::TryStreamExt};
use serde_json::Value;
use crate::{TABLE_NAME, DATABASE_NAME, index, redirector, get_conn, mount_v1};

async fn rocket_build() -> Rocket<Build> {
    let conn = match get_conn().await {
        Ok(conn) => conn,
        Err(_) => panic!("Can't connect to the database")
    };
    // create database if needed
    let mut query = r
        .db_create(DATABASE_NAME)
        .run::<_, Value>(&conn);
    if query.try_next().await.is_ok() {
        println!("Database created");
    }
    // create table if needed
    let mut query = r
        .db(DATABASE_NAME)
        .table_create(TABLE_NAME)
        .run::<_, Value>(&conn);
    if query.try_next().await.is_ok() {
        println!("Table created");
    }

    // build, mount and launch
    let rocket = rocket::build()
        .mount("/", routes![index])
        .mount("/r", routes![redirector]);
    let rocket = mount_v1(rocket);

    rocket
}

mod test {
    use rocket::local::asynchronous::Client;
    use rocket::http::Status;
    use serde_json::Value;
    use crate::tests::rocket_build;

    #[rocket::async_test]
    async fn get_redirect() {
        let client = Client::tracked(rocket_build().await).await.expect("valid rocket instance");
        let response = client.get("/r/test").dispatch().await;
        assert_eq!(response.status(), Status::SeeOther);
        assert_eq!(response.headers().get_one("Location"), Some("https://example.com"));
    }

    #[rocket::async_test]
    async fn list_of_redirects() {
        let client = Client::tracked(rocket_build().await).await.expect("valid rocket instance");
        let response = client.get("/api/v1/redirect").dispatch().await;
        assert_eq!(response.status(), Status::Ok);
        let example_list = "{\"success\":true,\"response\":[{\"domain\":\"https://example.com\",\"id\":1,\"name\":\"test\"}]}";
        let example_list: Value = serde_json::from_str(example_list).unwrap();
        let res: Value = serde_json::from_str(&response.into_string().await.unwrap()).unwrap();
        assert_eq!(example_list, res);
    }
}