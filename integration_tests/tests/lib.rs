mod sync;
mod dummy_jobs;
mod runner;
mod test_guard;

use coil::Job;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sqlx::Connection;
use std::sync::Once;
use once_cell::sync::Lazy;

static DATABASE_URL: Lazy<String> = Lazy::new(|| {
    dotenv::var("DATABASE_URL").unwrap()
});

static INIT: Once = Once::new();

pub fn initialize() {
    INIT.call_once(|| {
        let url = dotenv::var("DATABASE_URL").unwrap();
        println!("Running migrations...at {}", url);
        let mut conn = smol::block_on(sqlx::PgConnection::connect(url.as_str())).unwrap();
        smol::block_on(coil::migrate(&mut conn)).unwrap();
    });
}

struct Executor;
impl futures::task::Spawn for Executor {
    fn spawn_obj(&self, future: futures::task::FutureObj<'static, ()>) -> Result<(), futures::task::SpawnError> {
        smol::Task::spawn(future).detach();
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct Size {
    height: u32,
    width: u32,
}

pub struct Environment {
    conn: sqlx::PgPool,
}

#[coil::background_job]
async fn resize_image_async(to_sleep: u64) -> Result<(), coil::PerformError> {
    smol::Timer::new(std::time::Duration::from_millis(to_sleep)).await;
    Ok(())
}

#[coil::background_job]
fn resize_image(name: String) -> Result<(), coil::PerformError> {
    println!("{}", name);
    Ok(())
}

#[coil::background_job]
fn resize_image_gen<E: Serialize + DeserializeOwned + Send + std::fmt::Display>(some: E) -> Result<(), coil::PerformError> {
    println!("{}", some);
    Ok(())
}

#[test]
fn enqueue_5_jobs_limited_size() {
    initialize();
    let pool = smol::block_on(sqlx::PgPool::connect(&DATABASE_URL))
    .unwrap();

    smol::run(async move {
        resize_image("tohru".to_string()).enqueue(&pool).await.unwrap();
        resize_image("gambit".to_string()).enqueue(&pool).await.unwrap();
        resize_image("chess".to_string()).enqueue(&pool).await.unwrap();
        resize_image("kaguya".to_string()).enqueue(&pool).await.unwrap();
        resize_image("L".to_string()).enqueue(&pool).await.unwrap();
        resize_image("sinks".to_string()).enqueue(&pool).await.unwrap();
        resize_image("polkadotstingray".to_string()).enqueue(&pool).await.unwrap();
        resize_image("zutomayo".to_string()).enqueue(&pool).await.unwrap();
        resize_image("zzz".to_string()).enqueue(&pool).await.unwrap();
        resize_image("xix".to_string()).enqueue(&pool).await.unwrap();

        let runner = coil::Builder::new((), Executor, pool)
            .num_threads(8)
            .max_tasks(3)
            .build()
            .unwrap();
        runner.run_all_sync_tasks().await.unwrap();
    });
}

#[test]
fn enqueue_5_jobs_generic() {
    initialize();
    let pool = smol::block_on(sqlx::PgPool::connect(&DATABASE_URL))
    .unwrap();

    smol::run(async move {
        resize_image_gen("yuru".to_string()).enqueue(&pool).await.unwrap();
        resize_image_gen("100gecs".to_string()).enqueue(&pool).await.unwrap();
        resize_image_gen("papooz".to_string()).enqueue(&pool).await.unwrap();
        resize_image_gen("kaguya".to_string()).enqueue(&pool).await.unwrap();
        resize_image_gen("L".to_string()).enqueue(&pool).await.unwrap();

        let runner = coil::Builder::new((), Executor, pool)
            .num_threads(8)
            .max_tasks(3)
            .register_job::<resize_image_gen::Job<String>>()
            .build()
            .unwrap();

        runner.run_all_sync_tasks().await.unwrap();
    });
}
