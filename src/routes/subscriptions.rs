use ::actix_web::{web, HttpResponse};
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{NewSubscriber, SubscriberName,SubscriberEmail};

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

    let new_subscriber =  match  form.0.try_into() {
        Ok(v) => v,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };     
    
     match insert_subscriber(&new_subscriber,&pool).await
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
    skip(new_subscriber,pool),
)]
pub async fn insert_subscriber(new_subscriber: &NewSubscriber, pool: &PgPool) -> Result<(),sqlx::Error > {
  sqlx::query!(
        r#"
            INSERT INTO subscriptions (id, email ,name , subscribed_at) VALUES ($1,$2,$3,$4)
        "#,
        Uuid::new_v4(),
        new_subscriber.email.as_ref(),
        new_subscriber.name.as_ref(),
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


impl TryFrom<FormData> for NewSubscriber {
    type Error = String;

    fn try_from(value: FormData) -> Result<Self, Self::Error> {
       let name = SubscriberName::parse(value.name)?;
       let email = SubscriberEmail::parse(value.email)?;
       Ok(Self{ email,name })  
    }
}