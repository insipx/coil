
use serde::{Serialize, Deserialize};
use coil::Job;

#[test]
fn it_works() {
    assert_eq!(2 + 2, 4);
}

#[derive(Serialize, Deserialize)]
struct Size {
    height: u32,
    width: u32,
}

pub struct Environment {
    conn: sqlx::PgPool
}

#[coil::background_job]
fn resize_image_sync(file_name: String, dimensions: Size) -> Result<(), coil::PerformError> {
    println!("Hello");
    Ok(())
}

#[coil::background_job] 
async fn resize_image(file_name: String, dimensions: Size) -> Result<(), coil::PerformError> {
    println!("Hello");
    Ok(())
}

#[coil::background_job]
async fn resize_image_with_env(env: &Environment, file_name: String, dimensions: Size) -> Result<(), coil::PerformError> {
    println!("File Name: {}, height: {}, width: {}", file_name, dimensions.height, dimensions.width);
    Ok(())
}

struct Executor;

impl futures::task::Spawn for Executor {
    
    fn spawn_obj(&self, future: futures::task::FutureObj<'static, ()>) -> Result<(), futures::task::SpawnError> {
        smol::Task::spawn(future).detach();
        Ok(())
    }
}

#[test]
fn enqueue_simple_task() {
    let pool = smol::block_on(sqlx::PgPool::connect("postgres://archive:default@localhost:5432/test_job_queue")).unwrap();
    resize_image("Hello".to_string(), Size { height: 0, width: 0 }).enqueue(&pool);
    resize_image_sync("Hello".to_string(), Size { height: 0, width: 0 }).enqueue(&pool);
    resize_image_with_env("Hello".to_string(), Size { height: 0, width: 0}).enqueue(&pool);
}

#[test]
fn enqueue_5_jobs() {
    let pool = smol::block_on(sqlx::PgPool::connect("postgres://archive:default@localhost:5432/test_job_queue")).unwrap();
    let env = Environment {
        conn: pool.clone()
    };
    smol::run(async move {
        resize_image_with_env("tohru".to_string(), Size { height: 32, width: 32 }).enqueue(&pool).await.unwrap();
        resize_image_with_env("gambit".to_string(), Size { height: 64, width: 64 }).enqueue(&pool).await.unwrap();
        resize_image_with_env("chess".to_string(), Size { height: 128, width: 128 }).enqueue(&pool).await.unwrap();
        resize_image_with_env("kaguya".to_string(), Size { height: 256, width: 256 }).enqueue(&pool).await.unwrap();
        resize_image_with_env("L".to_string(), Size { height: 512, width: 512 }).enqueue(&pool).await.unwrap();

        let runner = coil::RunnerBuilder::new(env, Executor, pool)
            .num_threads(8)
            .build()
            .unwrap();
        runner.run_all_pending_tasks().await.unwrap();
        println!("Finished");
    });
    
}

