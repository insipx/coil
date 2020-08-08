
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
async fn resize_image_gen(file_name: String, dimensions: Size) -> Result<(), coil::PerformError> {
    println!("hello");
    Ok(())
}

#[coil::background_job]
async fn resize_image_with_env(env: &Environment, file_name: String, dimensions: Size) -> Result<(), coil::PerformError> {
    println!("Hello");
    Ok(())
}

#[test]
fn enqueue_simple_task() {
    let pool = smol::block_on(sqlx::PgPool::connect("postgres://archive:default@localhost:5432/test_job_queue")).unwrap();
    resize_image("Hello".to_string(), Size { height: 0, width: 0 }).enqueue(&pool);
    resize_image_sync("Hello".to_string(), Size { height: 0, width: 0 }).enqueue(&pool);
    resize_image_with_env("Hello".to_string(), Size { height: 0, width: 0}).enqueue(&pool);
}

