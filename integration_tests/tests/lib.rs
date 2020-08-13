use coil::Job;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

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
    conn: sqlx::PgPool,
}

/*
#[coil::background_job]
fn resize_image_sync(file_name: String, dimensions: Size) -> Result<(), coil::PerformError> {
    println!("Hello");
    Ok(())
}
*/
/*
#[coil::background_job]
async fn resize_image(file_name: String, dimensions: Size) -> Result<(), coil::PerformError> {
    println!("Hello");
    Ok(())
}
*/
/*
#[coil::background_job]
async fn resize_image<E: Serialize + DeserializeOwned + 'static + Send>(name: String, some: E) -> Result<(), coil::PerformError> {
    // println!("File Name: {}, height: {}, width: {}", file_name, dimensions.height, dimensions.width);
    println!("{}", name);
    Ok(())
}
*/

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

struct Executor;
impl futures::task::Spawn for Executor {
    fn spawn_obj(&self, future: futures::task::FutureObj<'static, ()>) -> Result<(), futures::task::SpawnError> {
        smol::Task::spawn(future).detach();
        Ok(())
    }
}

/*
#[test]
fn enqueue_simple_task() {
    let pool = smol::block_on(sqlx::PgPool::connect("postgres://archive:default@localhost:5432/test_job_queue")).unwrap();
    resize_image("Hello".to_string(), Size { height: 0, width: 0 }).enqueue(&pool);
    resize_image_sync("Hello".to_string(), Size { height: 0, width: 0 }).enqueue(&pool);
    resize_image("Hello".to_string(), Size { height: 0, width: 0}).enqueue(&pool);
}

#[test]
fn enqueue_5_jobs() {
    let pool = smol::block_on(sqlx::PgPool::connect("postgres://archive:default@localhost:5432/test_job_queue")).unwrap();
    let env = Environment {
        conn: pool.clone()
    };
    smol::run(async move {
        resize_image("tohru".to_string(), Size { height: 32, width: 32 }).enqueue(&pool).await.unwrap();
        resize_image("gambit".to_string(), Size { height: 64, width: 64 }).enqueue(&pool).await.unwrap();
        resize_image("chess".to_string(), Size { height: 128, width: 128 }).enqueue(&pool).await.unwrap();
        resize_image("kaguya".to_string(), Size { height: 256, width: 256 }).enqueue(&pool).await.unwrap();
        resize_image("L".to_string(), Size { height: 512, width: 512 }).enqueue(&pool).await.unwrap();

        let runner = coil::RunnerBuilder::new(env, Executor, pool)
            .num_threads(8)
            .build()
            .unwrap();
        runner.run_all_pending_tasks().await.unwrap();
        println!("Finished");
    });
}
*/

#[test]
fn enqueue_5_jobs_limited_size() {
    let pool = smol::block_on(sqlx::PgPool::connect(
        "postgres://archive:default@localhost:5432/test_job_queue",
    ))
    .unwrap();

    smol::run(async move {
        resize_image("tohru".to_string()).enqueue(&pool).await.unwrap();
        resize_image("gambit".to_string()).enqueue(&pool).await.unwrap();
        resize_image("chess".to_string()).enqueue(&pool).await.unwrap();
        resize_image("kaguya".to_string()).enqueue(&pool).await.unwrap();
        resize_image("L".to_string()).enqueue(&pool).await.unwrap();

        resize_image("sinks".to_string()).enqueue(&pool).await.unwrap();

        resize_image("polkadotstingray".to_string())
            .enqueue(&pool)
            .await
            .unwrap();

        resize_image("zutomayo".to_string()).enqueue(&pool).await.unwrap();

        resize_image("zzz".to_string()).enqueue(&pool).await.unwrap();

        resize_image("xix".to_string()).enqueue(&pool).await.unwrap();

        let runner = coil::RunnerBuilder::new((), Executor, pool)
            .num_threads(8)
            .max_tasks(3)
            .register_job::<resize_image::Job>()
            .build()
            .unwrap();
        runner.run_all_pending_tasks().await.unwrap();
        println!("Finished");
    });
}

#[test]
fn enqueue_5_jobs_generic() {
    let pool = smol::block_on(sqlx::PgPool::connect(
        "postgres://archive:default@localhost:5432/test_job_queue",
    ))
    .unwrap();

    smol::run(async move {
        resize_image_gen("yuru".to_string()).enqueue(&pool).await.unwrap();
        resize_image_gen("100gecs".to_string()).enqueue(&pool).await.unwrap();
        resize_image_gen("papooz".to_string()).enqueue(&pool).await.unwrap();
        resize_image_gen("kaguya".to_string()).enqueue(&pool).await.unwrap();
        resize_image_gen("L".to_string()).enqueue(&pool).await.unwrap();

        let runner = coil::RunnerBuilder::new((), Executor, pool)
            .num_threads(8)
            .max_tasks(3)
            .register_job::<resize_image_gen::Job<String>>()
            .build()
            .unwrap();

        runner.run_all_pending_tasks().await.unwrap();
        println!("Finished");
    });
}
