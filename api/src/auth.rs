use crate::error::AppError;
use crate::state::SharedState;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts, State},
    http::request::Parts,
    Json,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use sqlx::Row;
use chrono::{Utc, Duration};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: usize,
}

#[async_trait]
impl<S> FromRequestParts<S> for Claims
where
    S: Send + Sync,
    SharedState: axum::extract::FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(|_| AppError::Auth("Missing or invalid authorization header".to_string()))?;

        let app_state = SharedState::from_ref(state);
        
        let token_data = decode::<Claims>(
            bearer.token(),
            &DecodingKey::from_secret(app_state.jwt_secret.as_bytes()),
            &Validation::default(),
        )
        .map_err(|_| AppError::Auth("Invalid or expired token".to_string()))?;

        Ok(token_data.claims)
    }
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub display_name: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
}

pub async fn register(
    State(state): State<SharedState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    if payload.username.is_empty() || payload.password.is_empty() || payload.display_name.is_empty() {
        return Err(AppError::BadRequest("Fields cannot be empty".to_string()));
    }

    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let password_hash = argon2
        .hash_password(payload.password.as_bytes(), &salt)?
        .to_string();

    let user_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO users (username, display_name, password_hash)
        VALUES ($1, $2, $3)
        RETURNING id
        "#,
    )
    .bind(&payload.username)
    .bind(&payload.display_name)
    .bind(&password_hash)
    .fetch_one(&state.db)
    .await
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.constraint() == Some("users_username_key") {
                return AppError::BadRequest("Username already exists".to_string());
            }
        }
        AppError::from(e)
    })?;

    let token = generate_token(user_id, &state.jwt_secret)?;

    Ok(Json(AuthResponse { token }))
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn login(
    State(state): State<SharedState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    let row = sqlx::query(
        r#"
        SELECT id, password_hash
        FROM users
        WHERE username = $1
        "#,
    )
    .bind(&payload.username)
    .fetch_optional(&state.db)
    .await?;

    if let Some(row) = row {
        let id: Uuid = row.try_get("id")?;
        let hash: String = row.try_get("password_hash")?;

        let parsed_hash = PasswordHash::new(&hash)?;
        let argon2 = Argon2::default();
        if argon2
            .verify_password(payload.password.as_bytes(), &parsed_hash)
            .is_ok()
        {
            let token = generate_token(id, &state.jwt_secret)?;
            return Ok(Json(AuthResponse { token }));
        }
    }

    Err(AppError::Auth("Invalid username or password".to_string()))
}

#[derive(Serialize)]
pub struct UserProfile {
    pub id: Uuid,
    pub username: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
}

pub async fn me(
    claims: Claims,
    State(state): State<SharedState>,
) -> Result<Json<UserProfile>, AppError> {
    let profile = sqlx::query_as!(
        UserProfile,
        r#"
        SELECT id, username, display_name, avatar_url
        FROM users
        WHERE id = $1
        "#,
        claims.sub
    )
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Auth("User not found".to_string()))?;

    Ok(Json(profile))
}

fn generate_token(user_id: Uuid, secret: &str) -> Result<String, AppError> {
    let expiration = Utc::now()
        .checked_add_signed(Duration::days(7))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id,
        exp: expiration,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(e.into()))
}
