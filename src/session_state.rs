use std::future::{ready, Ready};

use actix_session::{Session, SessionExt};
use actix_web::FromRequest;
use uuid::Uuid;
pub struct TypeSession(Session);

impl TypeSession {
    const USER_ID_KEY: &'static str = "user_id";

    pub fn renew(&self) {
        self.0.renew();
    }

    pub fn logout(&self) {
        self.0.purge();
    }

    pub fn insert_user_id(&self, user_id: Uuid) -> Result<(), serde_json::Error> {
        self.0.insert(Self::USER_ID_KEY, user_id)
    }

    pub fn get_user_id(&self) -> Result<Option<Uuid>, serde_json::Error> {
        self.0.get(Self::USER_ID_KEY)
    }
}

impl FromRequest for TypeSession {
    type Error = <Session as FromRequest>::Error;

    type Future = Ready<Result<TypeSession, Self::Error>>;

    fn from_request(
        req: &actix_web::HttpRequest,
        _payload: &mut actix_web::dev::Payload,
    ) -> Self::Future {
        ready(Ok(TypeSession(req.get_session())))
    }
}
