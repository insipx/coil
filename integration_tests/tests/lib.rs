mod codegen;
mod dummy_jobs;
mod runner;
mod sync;
mod test_guard;

use crate::test_guard::TestGuard;
use coil::Job;
use once_cell::sync::Lazy;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sqlx::Connection;
use std::sync::Once;

static DATABASE_URL: Lazy<String> = Lazy::new(|| dotenv::var("DATABASE_URL").unwrap());

static INIT: Once = Once::new();

pub fn initialize() {
    INIT.call_once(|| {
        pretty_env_logger::init();
        let url = dotenv::var("DATABASE_URL").unwrap();
        let mut conn = smol::block_on(sqlx::PgConnection::connect(url.as_str())).unwrap();
        smol::block_on(coil::migrate(&mut conn)).unwrap();
    });
}

#[derive(Serialize, Deserialize)]
struct Size {
    height: u32,
    width: u32,
}

pub struct Environment {
    _conn: sqlx::PgPool,
}

#[coil::background_job]
fn resize_image(_name: String) -> Result<(), coil::PerformError> {
    std::thread::sleep(std::time::Duration::from_millis(150));
    Ok(())
}

#[coil::background_job]
fn resize_image_gen<E: Serialize + DeserializeOwned + Send + std::fmt::Display>(
    _some: E,
) -> Result<(), coil::PerformError> {
    Ok(())
}

#[test]
fn enqueue_8_jobs_limited_size() {
    initialize();
    let runner = TestGuard::runner(());
    log::info!("RUNNING `enqueue_8_jobs_limited_size`");

    let pool = runner.connection_pool();
    smol::block_on(async {
        resize_image("tohru".to_string())
            .enqueue(&pool)
            .await
            .unwrap();
        resize_image("gambit".to_string())
            .enqueue(&pool)
            .await
            .unwrap();
        resize_image("chess".to_string())
            .enqueue(&pool)
            .await
            .unwrap();
        resize_image("kaguya".to_string())
            .enqueue(&pool)
            .await
            .unwrap();
        resize_image("L".to_string()).enqueue(&pool).await.unwrap();
        resize_image("sinks".to_string())
            .enqueue(&pool)
            .await
            .unwrap();
        resize_image("polkadotstingray".to_string())
            .enqueue(&pool)
            .await
            .unwrap();
        resize_image("zutomayo".to_string())
            .enqueue(&pool)
            .await
            .unwrap();
    });

    smol::block_on(async move {
        runner.run_pending_tasks().unwrap();
        runner.check_for_failed_jobs().await.unwrap();
    });
}

#[test]
fn generic_jobs_can_be_enqueued() {
    initialize();
    let runner = TestGuard::builder(())
        .register_job::<resize_image_gen::Job<String>>()
        .build();
    log::info!("RUNNING `generic_jobs_can_be_enqueued`");
    let pool = runner.connection_pool();

    smol::block_on(async {
        resize_image_gen("yuru".to_string())
            .enqueue(&pool)
            .await
            .unwrap();
        resize_image_gen("100gecs".to_string())
            .enqueue(&pool)
            .await
            .unwrap();
        resize_image_gen("papooz".to_string())
            .enqueue(&pool)
            .await
            .unwrap();
        resize_image_gen("kaguya".to_string())
            .enqueue(&pool)
            .await
            .unwrap();
        resize_image_gen("L".to_string())
            .enqueue(&pool)
            .await
            .unwrap();
        runner.run_pending_tasks().unwrap();
        runner.check_for_failed_jobs().await.unwrap();
    });
}
