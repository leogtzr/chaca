/// Authentication implementation
use chrono::{Duration, Utc};
use jsonwebtoken::{
    decode, encode, errors::ErrorKind, DecodingKey, EncodingKey, Header, Validation,
};
use lazy_static::lazy_static;
use rocket::{
    http::Status, http::Cookie,
    request::{FromRequest, Outcome},
    response::status::Custom,
};
use serde::{Deserialize, Serialize};
use reqwest::Client;
use crate::db::*;
use rocket::serde::json::{Json, Value};
use rocket::serde::uuid::Uuid;

const BEARER: &str = "Bearer";
const AUTHORIZATION: &str = "Authorization";

// Define configuration structs
#[derive(Debug, Deserialize, Serialize)]
pub struct JwtConfig {
    pub secret: String,
    pub duration: i64,
}

lazy_static! {
    // Extract JWT configuration from the "jwt" table in Rocket.toml
    /// Time for token expiration
    static ref TOKEN_DURATION: Duration = Duration::minutes(get_token_duration());
}

/// Manage authentication decoding errors
#[derive(Debug, PartialEq)]
pub enum AuthenticationError {
    Missing,
    Decoding(String),
    Expired,
}

/// jsonwebtoken Claim

#[derive(Serialize, Deserialize, Debug)]
pub struct Claims {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub exp: usize, // expiration time
    pub iat: usize, // issued at time
}

// Facebook OAuth provider
pub struct Facebook;

// Create structs for user data and JWT claims
/// Fields to ask the Facebook graph API
//pub static FACEBOOK_FIELDS: &str = "id,email,name,short_name,picture,location";
pub static FACEBOOK_FIELDS: &str = "id,email,name,short_name,picture";

/// Derived from the info asked from the facebook graph API
#[derive(Debug, Deserialize, Serialize)]
pub struct FacebookUserInfo {
    pub id: String,
    pub email: Option<String>,
    pub name: String,
    pub short_name: String,
    pub picture: serde_json::Value,
    //pub location: String,
}

// Create a struct to hold application state
pub struct AppState {
    pub http_client: Client,
    pub jwt_secret: String,
}

/// Authenticated user for the frontend
#[derive(Debug, Serialize, Deserialize)]
pub struct AppUser {
    pub id: Uuid,
    pub oauth_id: String,
    pub name: String,
    pub email: Option<String>,
}

//Rocket request guard
#[rocket::async_trait]
impl<'r> FromRequest<'r> for Claims {
    type Error = AuthenticationError;
    async fn from_request(request: &'r rocket::Request<'_>) -> Outcome<Self, Self::Error> {
        // Get JWT secret from state - using the state managed by Rocket
        let state = request.rocket().state::<AppState>().unwrap();

        // Check if we have a cookie
        match request.cookies().get("auth_token") {
            Some(t) =>  match Claims::from_cookie(t, &state.jwt_secret) {
                Ok(c) => {
                    return Outcome::Success(c);
            },
                Err(e) => return Outcome::Error((Status::Forbidden, e)),
            },
            None => {} // Let the next match check for the token
        };

        match request.headers().get_one(AUTHORIZATION) {
            Some(v) => match Claims::from_authorization(v, &state.jwt_secret) {
                Ok(c) => Outcome::Success(c),
                Err(e) => Outcome::Error((Status::Forbidden, e)),
            },
            None => Outcome::Error((Status::Forbidden, AuthenticationError::Missing)),
        }
    }
}

/// Claims implementation
impl Claims {
    /// Creates a new claim with a given name
    pub fn from_name(name: &str) -> Self {
        Self {
            id: "".to_string(),
            name: name.to_string(),
            email: None,
            iat: 0,
            exp: 0,
        }
    }

    /// Create Claims from a 'Bearer <Token>' value
    fn from_authorization(value: &str, secret: &String) -> Result<Self, AuthenticationError> {
        let token = match value.strip_prefix(BEARER).map(str::trim) {
            Some(t) => t,
            None => return Err(AuthenticationError::Missing),
        };

        // Get claims from a JWT
        let token = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_ref()),
            &Validation::default(), //TODO check this defaults
        )
        .map_err(|e| match e.kind() {
            ErrorKind::ExpiredSignature => AuthenticationError::Expired,
            //TODO: check if we have different responses for each error
            _ => AuthenticationError::Decoding(e.to_string()),
        })?;

        Ok(token.claims)
    }

    /// Create Claims from a 'cookie' value
    pub fn from_cookie(cookie: &Cookie, secret: &String) -> Result<Self, AuthenticationError> {
        let token = String::from(cookie.value());

        // Get claims from a JWT
        let token = decode::<Claims>(
            &token,
            &DecodingKey::from_secret(secret.as_ref()),
            &Validation::default(), //TODO check this defaults
        )
        .map_err(|e| match e.kind() {
            ErrorKind::ExpiredSignature => AuthenticationError::Expired,
            //TODO: check if we have different responses for each error
            _ => AuthenticationError::Decoding(e.to_string()),
        })?;

        Ok(token.claims)
    }

    /// Convert this Claims instance to a token string to be sent to the browser
    pub fn into_token(mut self, secret: &String) -> Result<String, Custom<String>> {
        // the expiration of the token is: now + TOKEN_DURATION
        let now = Utc::now();
        // We store the time the token was issued
        self.iat = now.timestamp() as usize;
        let expiration = now
            .checked_add_signed(*TOKEN_DURATION)
            .expect("Failed to create expiration time")
            .timestamp();
        self.exp = expiration as usize;

        // Create the JWT
        let token = encode(
            &Header::default(),
            &self,
            &EncodingKey::from_secret(secret.as_ref()),
        )
        .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

        Ok(token)
    }

    /// Convert this Claims instance to a facebook token string to be sent to the browser
    pub fn into_facebook_token(mut self, user_data: &FacebookUserInfo, secret: &String) -> Result<String, Custom<String>> {

        self.id = user_data.id.clone();
        self.name = user_data.name.clone();
        self.email = user_data.email.clone();
        // the expiration of the token is: now + TOKEN_DURATION
        let now = Utc::now();
        // We store the time the token was issued
        self.iat = now.timestamp() as usize;
        let expiration = now
            .checked_add_signed(*TOKEN_DURATION)
            .expect("Failed to create expiration time")
            .timestamp();
        self.exp = expiration as usize;


        // Create the JWT
        let token = encode(
            &Header::default(),
            &self,
            &EncodingKey::from_secret(secret.as_ref()),
        )
        .map_err(|e| Custom(Status::BadRequest, e.to_string()))?;

        Ok(token)
    }

    /// Check if the auth token is valid
    /// TODO: maybe? remove
    pub fn is_logged(cookie: Option<&Cookie>, secret: &String) -> bool {
        // Check if we have a cookie
        match cookie {
            Some(t) =>  match Claims::from_cookie(&t, &secret) {
                Ok(c) => {
                    return true;
                },
                Err(e) => return false,
            },
            None => false
        };
        false
    }


}


/*******************************************************************************
*                                                                              *
*                                                                              *
*                     P R I V A T E  F U N C T I O N S                         *
*                                                                              *
*                                                                              *
********************************************************************************/


/// Get the duration of the token from the configuration file
fn get_token_duration() -> i64 {
    let figment = rocket::Config::figment();

    // Extract JWT configuration from the "jwt" table in Rocket.toml
    let jwt_config: JwtConfig = figment
        .extract_inner("jwt")
        .expect("JWT configuration missing in Rocket.toml");
    jwt_config.duration
}


