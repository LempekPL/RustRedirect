extern crate rocket;

use rocket::{Build, Rocket};
use crate::{index, redirector, mount_v1};
use crate::database::manage_database;

async fn rocket_build() -> Rocket<Build> {
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
    use crate::tests::rocket_build;

    // #[rocket::async_test]
    // async fn get_redirect() {
    //     let client = Client::tracked(rocket_build().await).await.expect("valid rocket instance");
    //     let response = client.get("/r/test").dispatch().await;
    //     assert_eq!(response.status(), Status::SeeOther);
    //     assert_eq!(response.headers().get_one("Location"), Some("https://example.com"));
    // }
    //
    // #[rocket::async_test]
    // async fn list_of_redirects() {
    //     let client = Client::tracked(rocket_build().await).await.expect("valid rocket instance");
    //     let response = client.get("/api/v1/redirect").dispatch().await;
    //     assert_eq!(response.status(), Status::Ok);
    //     let example_list = "{\"success\":true,\"response\":[{\"domain\":\"https://example.com\",\"id\":1,\"name\":\"test\"}]}";
    //     let example_list: Value = serde_json::from_str(example_list).unwrap();
    //     let res: Value = serde_json::from_str(&response.into_string().await.unwrap()).unwrap();
    //     assert_eq!(example_list, res);
    // }

    #[rocket::async_test]
    async fn create_redirect_delete() {
        let client = Client::tracked(rocket_build().await).await.expect("valid rocket instance");
        let res = client.post("/api/v1/redirect/create?name=test&domain=https://example.com")
            .header(ContentType::JSON)
            .body(r#"{"name": "admin", "password": "pass"}"#)
            .dispatch().await;
        assert_eq!(res.status(), Status::Ok);
        let res = client.get("/r/test").dispatch().await;
        assert_eq!(res.status(), Status::SeeOther);
        assert_eq!(res.headers().get_one("Location"), Some("https://example.com"));
        let res = client.delete("/api/v1/redirect/delete?name=test")
            .header(ContentType::JSON)
            .body(r#"{"name": "admin", "password": "pass"}"#)
            .dispatch().await;
        assert_eq!(res.status(), Status::Ok);
        let res: Value = serde_json::from_str(&res.into_string().await.unwrap()).unwrap();
        let example_list = r#"{"success":true,"response": "Deleted redirect named 'test'"}"#;
        let example_list: Value = serde_json::from_str(example_list).unwrap();
        assert_eq!(res, example_list);
    }

    #[rocket::async_test]
    async fn create_edit_delete() {

    }

    #[rocket::async_test]
    async fn create_edit_redirect_delete() {

    }

    #[rocket::async_test]
    async fn create_more_edit_list_delete() {

    }
}