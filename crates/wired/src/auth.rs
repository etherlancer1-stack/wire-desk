//! Authentication: JWT sessions + Argon2id passwords + API keys (specs #148, #162).
//! Raw secrets never returned in responses (spec #300).

use crate::error::{WireError, WireResult};
use crate::models::{Role, utc_now};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use argon2::password_hash::{SaltString, rand_core::OsRng};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

const TOKEN_TTL_SECONDS: i64 = 7 * 24 * 3600; // 7 days

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,   // user id
    pub tenant_id: String,
    pub email: String,
    pub role: String,
    pub exp: i64,
    pub iat: i64,
}

pub struct Auth {
    jwt_secret: Vec<u8>,
}

impl Auth {
    pub fn new(jwt_secret: &str) -> Self {
        Self { jwt_secret: jwt_secret.as_bytes().to_vec() }
    }

    /// Hash a password with Argon2id (spec #148).
    pub fn hash_password(password: &str) -> WireResult<String> {
        let salt = SaltString::generate(OsRng);
        let argon2 = Argon2::default();
        let hash = argon2.hash_password(password.as_bytes(), &salt)
            .map_err(|e| WireError::Internal(format!("argon2 hash failed: {}", e)))?;
        Ok(hash.to_string())
    }

    /// Verify a password against an Argon2id hash.
    pub fn verify_password(password: &str, hash: &str) -> WireResult<bool> {
        let argon2 = Argon2::default();
        let parsed_hash = argon2::password_hash::PasswordHash::new(hash)
            .map_err(|e| WireError::Internal(format!("parse hash failed: {}", e)))?;
        Ok(argon2.verify_password(password.as_bytes(), &parsed_hash).is_ok())
    }

    /// Issue a JWT session token.
    pub fn issue_token(&self, user_id: &str, tenant_id: &str, email: &str, role: &str) -> WireResult<String> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
        let claims = Claims {
            sub: user_id.to_string(),
            tenant_id: tenant_id.to_string(),
            email: email.to_string(),
            role: role.to_string(),
            exp: now + TOKEN_TTL_SECONDS,
            iat: now,
        };
        encode(&Header::default(), &claims, &EncodingKey::from_secret(&self.jwt_secret))
            .map_err(|e| WireError::Internal(format!("jwt encode failed: {}", e)))
    }

    /// Verify and decode a JWT token.
    pub fn verify_token(&self, token: &str) -> WireResult<Claims> {
        let validation = Validation::default();
        let token_data = decode::<Claims>(token, &DecodingKey::from_secret(&self.jwt_secret), &validation)
            .map_err(|_| WireError::Unauthorized)?;
        Ok(token_data.claims)
    }

    /// Hash an API key with SHA-256, storing a non-secret prefix (spec #148).
    pub fn hash_api_key(key: &str) -> (String, String) {
        let prefix = key.get(..8).unwrap_or("wd_live").to_string();
        let digest = sha2::Sha256::digest(key.as_bytes());
        let hash = hex::encode(digest);
        (prefix, hash)
    }

    /// Verify an API key against its stored hash.
    pub fn verify_api_key(key: &str, prefix: &str, hash: &str) -> bool {
        let (p, h) = Self::hash_api_key(key);
        p == prefix && h == hash
    }

    pub fn require_role(user_role: &str, required: &[Role]) -> WireResult<()> {
        let role = match user_role {
            "owner" => Role::Owner,
            "admin" => Role::Admin,
            "analyst" => Role::Analyst,
            "viewer" => Role::Viewer,
            _ => return Err(WireError::Forbidden("invalid role".into())),
        };
        if required.contains(&role) {
            Ok(())
        } else {
            Err(WireError::Forbidden("insufficient role".into()))
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: String,
    pub tenant_id: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyResponse {
    pub id: String,
    pub prefix: String,
    pub plaintext: Option<String>, // only shown on creation
}
