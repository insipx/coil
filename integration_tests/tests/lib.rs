
use serde::{Serialize, Deserialize};

#[test]
fn it_works() {
    assert_eq!(2 + 2, 4);
}

#[derive(Serialize, Deserialize)]
struct Size {
    height: u32,
    width: u32,
}

#[coil::background_job] 
fn resize_image(file_name: String, dimensions: Size) -> Result<(), coil::PerformError> {
    println!("Hello");
    Ok(())
}
