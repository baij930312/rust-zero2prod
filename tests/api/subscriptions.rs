use crate::helpers::spawn_app;

#[tokio::test]
async fn subcribe_returns_a_200_for_valid_from_data() {
    let app = spawn_app().await;

    let body = "name=bai%20jin&email=baij930312@163.com";
    let response = app.post_subscriptions(body.into()).await;

    assert_eq!(200, response.status().as_u16());
    let saved = sqlx::query!("SELECT email, name FROM subscriptions",)
        .fetch_one(&app.db_pool.clone())
        .await
        .expect("Faild to fetch saved subscription");

    assert_eq!(saved.email, "baij930312@163.com");
    assert_eq!(saved.name, "bai jin");
}

#[tokio::test]
async fn subcribe_returns_a_400_when_a_data_is_missing() {
    let app = spawn_app().await;

    let test_case = vec![
        ("name=bai", "missing the email"),
        ("email=baij930312@163.com", "missing the name"),
        ("", "missing both name and email"),
    ];
    for (body, msg) in test_case {
        let response = app.post_subscriptions(body.into()).await;
        assert_eq!(
            400,
            response.status().as_u16(),
            "The Api did not fail with 400 Bad Request when the payload was {}",
            msg
        );
    }
}

#[tokio::test]
async fn subcribe_returns_a_200_when_fields_are_present_but_empty() {
    let app = spawn_app().await;

    let test_case = vec![
        ("name=&bai@asd.com", "empty name"),
        ("name=asdasdsa&email= ", "empty email"),
        ("ame=asdasdsa&email=asdadsa2", "invalid email"),
    ];
    for (body, msg) in test_case {
        let response = app.post_subscriptions(body.into()).await;
        assert_eq!(
            400,
            response.status().as_u16(),
            "The Api did not fail with 400 Bad Request when the payload was {}",
            msg
        );
    }
}
