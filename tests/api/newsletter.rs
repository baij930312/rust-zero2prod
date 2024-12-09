use wiremock::{
    matchers::{any, method, path},
    Mock, ResponseTemplate,
};

use crate::helpers::{spawn_app, ConfirmationLinks, TestApp};

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    let app = spawn_app().await;
    create_unconfirmed_subscriber(&app).await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let nesletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "content":{
            "text":"Newsletter bodu as plain text",
            "html":"<p>Newsletter bodu as html</p>",
        }
    });

    let response = app.post_newsletter(nesletter_request_body).await;

    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let nesletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "content":{
            "text":"Newsletter bodu as plain text",
            "html":"<p>Newsletter bodu as html</p>",
        }
    });
    let response = app.post_newsletter(nesletter_request_body).await;

    assert_eq!(200, response.status().as_u16());
}

#[tokio::test]
async fn newsletters_returns_400_for_invalid_data() {
    let app = spawn_app().await;
    let test_case = vec![(
        serde_json::json!({
            "content":{
                "text":"Newsletter body as plain text",
                "html":"<p>Newsletter body as html</p>",
            }
        }),
        "missing title",
    )];

    for (body, msg) in test_case {
        let response = app.post_newsletter(body).await;
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            msg
        );
    }
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let body = "name=bai%20jin&email=baij930312@163.com";
    let _mock_guard = Mock::given(path("/email"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;
    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();
    app.get_confirmation_links(email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_link = create_unconfirmed_subscriber(&app).await;
    reqwest::get(confirmation_link.html)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}
