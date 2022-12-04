use std::sync::Arc;

use crate::config::Config;
use axum::{
    http::{self, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use eyre::eyre;
use eyre::Result;
use jwt_simple::prelude::*;

#[derive(Serialize, Deserialize)]
struct CustomClaims {
    slicing: bool,
}

pub async fn auth<B>(
    config: Arc<Config>,
    req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    let auth_header = req
        .headers()
        .get(http::header::AUTHORIZATION)
        .and_then(|header| header.to_str().ok());

    match auth_header {
        Some(auth_header) if token_is_valid(config, auth_header).is_ok() => Ok(next.run(req).await),
        _ => Err(StatusCode::UNAUTHORIZED),
    }
}

fn token_is_valid(config: Arc<Config>, auth_header: &str) -> Result<()> {
    // Verify that the authorization header contains a bearer token
    const BEARER: &'static str = "Bearer ";

    if !auth_header.starts_with(BEARER) {
        return Err(eyre!("Invalid bearer auth"));
    }

    let bearer_token = auth_header.replacen(BEARER, "", 1);

    // Verify that the bearer token is a authorized jwt
    let metadata = jwt_simple::token::Token::decode_metadata(&bearer_token)
        .map_err(|_| eyre!("Invalid JWT metadata"))?;

    let key_id = metadata
        .key_id()
        .ok_or_else(|| eyre!("Missing JWT key id (kid)"))?;

    let public_key_pem = &config
        .authorized_keys
        .get(key_id)
        .ok_or_else(|| eyre!("Unauthorized JWT key id"))?
        .public_key_pem;

    let public_key = ES256KeyPair::from_pem(&public_key_pem)
        .map_err(|_| eyre!("Invalid public key configured"))?
        .public_key();

    let claims = public_key
        .verify_token::<CustomClaims>(&bearer_token, None)
        .map_err(|_| eyre!("Invalid JWT"))?;

    if claims.custom.slicing != true {
        return Err(eyre!(
            "slicing must be set to true in the JWT to authorize slicing server access"
        ));
    }

    Ok(())
}
