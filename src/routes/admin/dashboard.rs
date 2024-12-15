use ::actix_web::HttpResponse;
use actix_web::{http::header::ContentType, web};

use anyhow::Context;
use reqwest::header::LOCATION;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{session_state::TypeSession, utils::e500};

pub async fn admin_dashboard(
    session: TypeSession,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, actix_web::Error> {
    let username = if let Some(user_id) = session.get_user_id().map_err(e500)? {
        get_username(user_id, &pool).await.map_err(e500)?
    } else {
        return Ok(HttpResponse::SeeOther()
            .insert_header((LOCATION, format!("/login")))
            .finish());
    };

    Ok(HttpResponse::Ok()
        .content_type(ContentType::html())
        .body(format!(
            r#"
        <!DOCTYPE html>
<html lang="en">
 
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Admin dashboard</title>
</head>

<body>
        <p>Welcome {username}</p>
        <p>Avaliable actions: </p>
        <ol>
            <li><a href="/admin/password">Change password</a></li>
            <li>
                 <form name="logoutForm" action="/admin/logout" method="post">
                    <input type="submit" value="Logout"/>
                </form>
            </li>
        </ol>
   

</body>
</html>
        "#
        )))
}

#[tracing::instrument(name = "Get username", skip(pool))]
pub async fn get_username(user_id: Uuid, pool: &PgPool) -> Result<String, anyhow::Error> {
    let row = sqlx::query!(
        r#"
        SELECT username
        FROM users 
        WHERE user_id = $1
        "#,
        user_id
    )
    .fetch_one(pool)
    .await
    .context("Failed to perform a query to retrieve a username.")?;

    Ok(row.username)
}
