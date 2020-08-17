pub use coil::Job;
use coil::PerformError;
use crate::sync::Barrier;



#[coil::background_job]
pub fn barrier_job(env: &Barrier) -> Result<(), PerformError> {
    env.wait();
    Ok(())
}

#[coil::background_job]
pub fn failure_job() -> Result<(), PerformError> {
    Err(PerformError::General("failure".into()))
}

#[coil::background_job]
pub fn panic_job() -> Result<(), PerformError> {
    panic!()
}
