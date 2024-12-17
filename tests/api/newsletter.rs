use fake::{
    faker::{internet::en::SafeEmail, name::en::Name},
    Fake,
};
use wiremock::{
    matchers::{any, method, path},
    Mock, MockBuilder, ResponseTemplate,
};

use crate::helpers::{assert_is_redirect_to, spawn_app, ConfirmationLinks, TestApp};

#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers() {
    let app = spawn_app().await;
    let login_body = serde_json::json!({
        "username":&app.test_user.username,
        "password":&app.test_user.password,
    });

    app.post_login(&login_body).await;

    create_unconfirmed_subscriber(&app).await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let nesletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "text_content": "Newsletter bodu as plain text",
        "html_content": "<p>Newsletter bodu as html</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });

    let response = app.post_newsletter(&nesletter_request_body).await;

    assert_is_redirect_to(&response, "/admin/newsletters");
}

#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers() {
    let app = spawn_app().await;
    let login_body = serde_json::json!({
        "username":&app.test_user.username,
        "password":&app.test_user.password,
    });

    app.post_login(&login_body).await;

    create_confirmed_subscriber(&app).await;
    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let nesletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "text_content": "Newsletter bodu as plain text",
        "html_content": "<p>Newsletter bodu as html</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });
    let response = app.post_newsletter(&nesletter_request_body).await;

    assert_is_redirect_to(&response, "/admin/newsletters");
}

#[tokio::test]
async fn newsletters_returns_400_for_invalid_data() {
    let app = spawn_app().await;
    let login_body = serde_json::json!({
        "username":&app.test_user.username,
        "password":&app.test_user.password,
    });

    app.post_login(&login_body).await;

    let test_case = vec![(
        serde_json::json!({
            "text_content": "Newsletter bodu as plain text",
            "html_content": "<p>Newsletter bodu as html</p>",
        }),
        "missing title",
    )];

    for (body, msg) in test_case {
        let response = app.post_newsletter(&body).await;
        assert_eq!(
            400,
            response.status().as_u16(),
            "The API did not fail with 400 Bad Request when the payload was {}.",
            msg
        );
    }
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> ConfirmationLinks {
    let name: String = Name().fake();
    let email: String = SafeEmail().fake();
    let body = serde_urlencoded::to_string(&serde_json::json!({
        "name":name,
        "email":email,
    }))
    .unwrap();
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

#[tokio::test]
async fn newsletter_creation_is_idempotent() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    let login_body = serde_json::json!({
        "username":&app.test_user.username,
        "password":&app.test_user.password,
    });
    app.post_login(&login_body).await;

    Mock::given(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let nesletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "text_content": "Newsletter bodu as plain text",
        "html_content": "<p>Newsletter bodu as html</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });
    let response = app.post_newsletter(&nesletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_newsletter_html().await;
    assert!(html_page.contains(r#"<p><i>The newsletter issue has been published!</i></p>"#));

    let response = app.post_newsletter(&nesletter_request_body).await;
    assert_is_redirect_to(&response, "/admin/newsletters");

    let html_page = app.get_newsletter_html().await;
    assert!(html_page.contains(r#"<p><i>The newsletter issue has been published!</i></p>"#));
}

#[tokio::test]
async fn concurrent_from_submission_is_handled_gracefully() {
    let app = spawn_app().await;
    create_confirmed_subscriber(&app).await;
    let login_body = serde_json::json!({
        "username":&app.test_user.username,
        "password":&app.test_user.password,
    });
    app.post_login(&login_body).await;

    Mock::given(path("/email"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    let nesletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "text_content": "Newsletter bodu as plain text",
        "html_content": "<p>Newsletter bodu as html</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });
    let response1 = app.post_newsletter(&nesletter_request_body);
    let response2 = app.post_newsletter(&nesletter_request_body);
    let (response1, response2) = tokio::join!(response1, response2);
    assert_eq!(response1.status(), response2.status());
    assert_eq!(
        response1.text().await.unwrap(),
        response2.text().await.unwrap()
    );
}

#[tokio::test]
async fn transient_errors_do_not_cause_duplicate_deliveries_on_retries() {
    let app = spawn_app().await;
    let login_body = serde_json::json!({
        "username":&app.test_user.username,
        "password":&app.test_user.password,
    });

    let nesletter_request_body = serde_json::json!({
        "title":"Newsletter title",
        "text_content": "Newsletter bodu as plain text",
        "html_content": "<p>Newsletter bodu as html</p>",
        "idempotency_key": uuid::Uuid::new_v4().to_string(),
    });
    create_confirmed_subscriber(&app).await;
    create_confirmed_subscriber(&app).await;
    app.post_login(&login_body).await;

    when_sending_an_email()
        .respond_with(ResponseTemplate::new(200))
        .up_to_n_times(1)
        .expect(1)
        .mount(&app.email_server)
        .await;

    when_sending_an_email()
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .expect(1)
        .mount(&app.email_server)
        .await;

    let response = app.post_newsletter(&nesletter_request_body).await;
    assert_eq!(response.status().as_u16(), 500);

    when_sending_an_email()
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .named("Delivey retry")
        .mount(&app.email_server)
        .await;

    let response = app.post_newsletter(&nesletter_request_body).await;
    assert_eq!(response.status().as_u16(), 303);
}

fn when_sending_an_email() -> MockBuilder {
    Mock::given(path("/email")).and(method("POST"))
}
