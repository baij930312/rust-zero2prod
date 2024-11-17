use ::actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

#[tracing::instrument(
    name = "Adding   as a new  subscriber",
    skip(form,pool),
    fields(
 
        email= %form.email,
        name= %form.name 
    )
)]
pub async fn subscriptions(form: web::Form<FormData>, pool: web::Data<PgPool>) -> HttpResponse {
     match insert_subscriber(&form,&pool).await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            tracing::error!("Failed to execute query :{:?}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[tracing::instrument(
    name = "Saving as a new  subscriber",
    skip(form,pool),
)]
pub async fn insert_subscriber(form: &FormData, pool: &PgPool) -> Result<(),sqlx::Error > {
  sqlx::query!(
        r#"
            INSERT INTO subscriptions (id, email ,name , subscribed_at) VALUES ($1,$2,$3,$4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        Utc::now(), 
    )
    .execute(pool)
    .await
    .map_err(|e|{
        tracing::error!("Faild to execute query : {:?}",e);
        e
    })?;
    
    Ok(())
}
