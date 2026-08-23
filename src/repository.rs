use std::convert::Infallible;

use axum::extract::FromRequestParts;
use sqlx::PgPool;

use crate::{
    app::AppState,
    error::AppError,
    models::{Asset, HoldingPosition, UserRecord},
};

pub struct Repository {
    db: PgPool,
}

impl Repository {
    pub async fn list_assets(&self) -> sqlx::Result<Vec<Asset>> {
        sqlx::query_as!(
            Asset,
            "SELECT id, name, unit_value
             FROM assets;"
        )
        .fetch_all(&self.db)
        .await
    }

    pub async fn create_asset(&self, name: String, unit_value: f64) -> sqlx::Result<Asset> {
        sqlx::query_as!(
            Asset,
            "INSERT INTO assets (name, unit_value)
             VALUES ($1, $2)
             RETURNING id, name, unit_value;",
            name,
            unit_value
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn update_asset(
        &self,
        asset_id: i64,
        name: Option<String>,
        unit_value: Option<f64>,
    ) -> sqlx::Result<Option<Asset>> {
        sqlx::query_as!(
            Asset,
            "UPDATE assets
             SET name=COALESCE($2, name),
                 unit_value=COALESCE($3, unit_value)
             WHERE id=$1
             RETURNING id, name, unit_value;",
            asset_id,
            name,
            unit_value
        )
        .fetch_optional(&self.db)
        .await
    }

    pub async fn add_user(&self, username: &str, password_hash: &str) -> sqlx::Result<UserRecord> {
        sqlx::query_as!(
            UserRecord,
            "INSERT INTO users (username, password_hash)
             VALUES ($1, $2)
             RETURNING id, username, password_hash;",
            username,
            password_hash,
        )
        .fetch_one(&self.db)
        .await
    }

    pub async fn get_user_by_name(&self, username: &str) -> sqlx::Result<Option<UserRecord>> {
        sqlx::query_as!(
            UserRecord,
            "SELECT id, username, password_hash
             FROM users
             WHERE username = $1;",
            username
        )
        .fetch_optional(&self.db)
        .await
    }

    pub async fn list_holdings(&self, user_id: i64) -> sqlx::Result<Vec<HoldingPosition>> {
        sqlx::query_as!(
            HoldingPosition,
            "SELECT h.id, h.quantity, a.id AS asset_id, a.name, a.unit_value
             FROM holdings h
             JOIN assets a ON a.id = h.asset_id
             WHERE h.user_id = $1
             ORDER BY a.name;",
            user_id
        )
        .fetch_all(&self.db)
        .await
    }

    pub async fn add_holding(
        &self,
        user_id: i64,
        asset_id: i64,
        quantity: f64,
    ) -> Result<(), AppError> {
        if quantity <= 0.0 {
            return Err(AppError::InvalidQuantity);
        }

        let result = sqlx::query!(
            "INSERT INTO holdings (user_id, asset_id, quantity)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id, asset_id)
             DO UPDATE SET quantity = holdings.quantity + EXCLUDED.quantity",
            user_id,
            asset_id,
            quantity
        )
        .execute(&self.db)
        .await;

        match result {
            Ok(_) => Ok(()),
            Err(sqlx::Error::Database(db_err)) if db_err.is_foreign_key_violation() => {
                Err(AppError::AssetDoesNotExist)
            }
            Err(err) => Err(AppError::Database(err)),
        }
    }

    pub async fn update_holding_quantity(
        &self,
        user_id: i64,
        holding_id: i64,
        quantity: f64,
    ) -> Result<Option<HoldingPosition>, AppError> {
        if quantity <= 0.0 {
            return Err(AppError::InvalidQuantity);
        }

        sqlx::query_as!(
            HoldingPosition,
            "UPDATE holdings h
             SET quantity = $3
             FROM assets a
             WHERE h.id = $1
               AND h.user_id = $2
               AND a.id = h.asset_id
             RETURNING h.id, h.quantity, a.id AS asset_id, a.name, a.unit_value;",
            holding_id,
            user_id,
            quantity
        )
        .fetch_optional(&self.db)
        .await
        .map_err(AppError::Database)
    }

    pub async fn delete_holding(&self, user_id: i64, holding_id: i64) -> Result<bool, AppError> {
        let result = sqlx::query!(
            "DELETE FROM holdings
             WHERE id = $1 AND user_id = $2;",
            holding_id,
            user_id
        )
        .execute(&self.db)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}

impl FromRequestParts<AppState> for Repository {
    type Rejection = Infallible;

    async fn from_request_parts(
        _parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        Ok(Self {
            db: state.db.clone(),
        })
    }
}

#[cfg(test)]
impl From<PgPool> for Repository {
    fn from(db: PgPool) -> Self {
        Self { db }
    }
}

#[cfg(test)]
mod tests {
    use sqlx::PgPool;

    use super::*;

    async fn seed_user_and_assets(repository: &Repository) -> (i64, i64, i64) {
        let user = repository
            .add_user("alice", "hash")
            .await
            .expect("user");
        let bitcoin = repository
            .create_asset("Bitcoin".to_string(), 10.0)
            .await
            .expect("bitcoin");
        let ethereum = repository
            .create_asset("Ethereum".to_string(), 20.0)
            .await
            .expect("ethereum");

        (user.id, bitcoin.id, ethereum.id)
    }

    #[sqlx::test]
    async fn add_holding_creates_position_and_subtotal(db: PgPool) {
        let repository = Repository::from(db);
        let (user_id, bitcoin_id, _) = seed_user_and_assets(&repository).await;

        repository
            .add_holding(user_id, bitcoin_id, 2.0)
            .await
            .expect("add holding");

        let holdings = repository.list_holdings(user_id).await.expect("list");
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].name, "Bitcoin");
        assert_eq!(holdings[0].quantity, 2.0);
        assert_eq!(holdings[0].subtotal(), 20.0);
    }

    #[sqlx::test]
    async fn add_same_asset_increases_quantity(db: PgPool) {
        let repository = Repository::from(db);
        let (user_id, bitcoin_id, _) = seed_user_and_assets(&repository).await;

        repository
            .add_holding(user_id, bitcoin_id, 1.0)
            .await
            .expect("first add");
        repository
            .add_holding(user_id, bitcoin_id, 3.0)
            .await
            .expect("second add");

        let holdings = repository.list_holdings(user_id).await.expect("list");
        assert_eq!(holdings.len(), 1);
        assert_eq!(holdings[0].quantity, 4.0);
        assert_eq!(holdings[0].subtotal(), 40.0);
    }

    #[sqlx::test]
    async fn portfolio_total_is_sum_of_subtotals(db: PgPool) {
        let repository = Repository::from(db);
        let (user_id, bitcoin_id, ethereum_id) = seed_user_and_assets(&repository).await;

        repository
            .add_holding(user_id, bitcoin_id, 2.0)
            .await
            .expect("bitcoin");
        repository
            .add_holding(user_id, ethereum_id, 1.5)
            .await
            .expect("ethereum");

        let holdings = repository.list_holdings(user_id).await.expect("list");
        let total: f64 = holdings.iter().map(HoldingPosition::subtotal).sum();
        assert_eq!(total, 50.0);
    }

    #[sqlx::test]
    async fn update_holding_quantity(db: PgPool) {
        let repository = Repository::from(db);
        let (user_id, bitcoin_id, _) = seed_user_and_assets(&repository).await;

        repository
            .add_holding(user_id, bitcoin_id, 2.0)
            .await
            .expect("add");
        let holding_id = repository.list_holdings(user_id).await.expect("list")[0].id;

        let updated = repository
            .update_holding_quantity(user_id, holding_id, 5.0)
            .await
            .expect("update")
            .expect("exists");

        assert_eq!(updated.quantity, 5.0);
        assert_eq!(updated.subtotal(), 50.0);
    }

    #[sqlx::test]
    async fn delete_holding(db: PgPool) {
        let repository = Repository::from(db);
        let (user_id, bitcoin_id, _) = seed_user_and_assets(&repository).await;

        repository
            .add_holding(user_id, bitcoin_id, 2.0)
            .await
            .expect("add");
        let holding_id = repository.list_holdings(user_id).await.expect("list")[0].id;

        let deleted = repository
            .delete_holding(user_id, holding_id)
            .await
            .expect("delete");
        assert!(deleted);

        let holdings = repository.list_holdings(user_id).await.expect("list");
        assert!(holdings.is_empty());
    }

    #[sqlx::test]
    async fn cannot_update_another_users_holding(db: PgPool) {
        let repository = Repository::from(db);
        let (alice_id, bitcoin_id, _) = seed_user_and_assets(&repository).await;
        let bob = repository.add_user("bob", "hash").await.expect("bob");

        repository
            .add_holding(alice_id, bitcoin_id, 2.0)
            .await
            .expect("add");
        let holding_id = repository.list_holdings(alice_id).await.expect("list")[0].id;

        let updated = repository
            .update_holding_quantity(bob.id, holding_id, 99.0)
            .await
            .expect("update");
        assert!(updated.is_none());

        let alice_holdings = repository.list_holdings(alice_id).await.expect("list");
        assert_eq!(alice_holdings[0].quantity, 2.0);
    }

    #[sqlx::test]
    async fn rejects_non_positive_quantity(db: PgPool) {
        let repository = Repository::from(db);
        let (user_id, bitcoin_id, _) = seed_user_and_assets(&repository).await;

        let err = repository
            .add_holding(user_id, bitcoin_id, 0.0)
            .await
            .expect_err("zero");
        assert!(matches!(err, AppError::InvalidQuantity));
    }

    #[sqlx::test]
    async fn rejects_unknown_asset(db: PgPool) {
        let repository = Repository::from(db);
        let user = repository.add_user("alice", "hash").await.expect("user");

        let err = repository
            .add_holding(user.id, 999, 1.0)
            .await
            .expect_err("missing asset");
        assert!(matches!(err, AppError::AssetDoesNotExist));
    }
}
