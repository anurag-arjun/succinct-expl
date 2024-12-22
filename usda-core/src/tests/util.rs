use sqlx::PgPool;

#[allow(dead_code)]
pub async fn clear_database(pool: &PgPool) {
    sqlx::query!("DELETE FROM transactions")
        .execute(pool)
        .await
        .expect("Failed to clear transactions");
        
    sqlx::query!("DELETE FROM accounts")
        .execute(pool)
        .await
        .expect("Failed to clear accounts");
        
    sqlx::query!("DELETE FROM proof_batches")
        .execute(pool)
        .await
        .expect("Failed to clear proof batches");
}

#[allow(dead_code)]
pub async fn setup_test_database(pool: &PgPool) {
    // Clear existing data
    clear_database(pool).await;
    
    // Run migrations
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .expect("Failed to run migrations");
}
