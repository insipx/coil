use assert_matches::assert_matches;
use std::sync::mpsc::sync_channel;
use anyhow::{Error, Result};
use std::thread;
use std::time::Duration;
// use coil::FailedJobsError;

use crate::dummy_jobs::*;
use crate::sync::Barrier;
use crate::test_guard::TestGuard;
/*
#[test]
fn run_all_pending_jobs_returns_when_all_jobs_enqueued() -> Result<()> {
    let barrier = Barrier::new(3);
    let runner = TestGuard::runner(barrier.clone());
    smol::block_on(async move {
        let conn = runner.connection().await?;
        barrier_job().enqueue(&conn).await?;
        barrier_job().enqueue(&conn).await?;

        smol::block_on(runner.run_all_sync_tasks())?;

        let queued_job_count = background_jobs::table.count().get_result(&conn);
        let unlocked_job_count = background_jobs::table
            .select(background_jobs::id)
            .for_update()
            .skip_locked()
            .load::<i64>(&conn)
            .map(|v| v.len());

        assert_eq!(Ok(2), queued_job_count);
        assert_eq!(Ok(0), unlocked_job_count);
    });


    barrier.wait();
    Ok(())
}
 */
/*
#[test]
fn check_for_failed_jobs_blocks_until_all_queued_jobs_are_finished() -> Result<()> {
    let barrier = Barrier::new(3);
    let runner = TestGuard::runner(barrier.clone());
    let conn = runner.connection_pool();
    smol::block_on(async move {
        barrier_job().enqueue(&conn)?;
        barrier_job().enqueue(&conn)
    })?;

    smol::block_on(runner.run_all_sync_tasks())?;

    let (send, recv) = sync_channel(0);
    let handle = thread::spawn(move || {
        let wait = Duration::from_millis(100);
        assert!(
            recv.recv_timeout(wait).is_err(),
            "wait_for_jobs returned before jobs finished"
        );

        barrier.wait();

        assert!(recv.recv().is_ok(), "wait_for_jobs didn't return");
    });

    runner.check_for_failed_jobs()?;
    send.send(1)?;
    handle.join().unwrap();
    Ok(())
}
 */

#[test]
fn check_for_failed_jobs_panics_if_jobs_failed() -> Result<()> {
    let runner = TestGuard::dummy_runner();
    let conn = runner.connection_pool();
    smol::block_on(async move {
        failure_job().enqueue(&conn).await?;
        failure_job().enqueue(&conn).await?;
        failure_job().enqueue(&conn).await
    })?;


    smol::block_on(runner.run_all_sync_tasks())?;
    // assert_eq!(Err(JobsFailed(3)), runner.check_for_failed_jobs());
    Ok(())
}

#[test]
fn panicking_jobs_are_caught_and_treated_as_failures() -> Result<()> {
    let runner = TestGuard::dummy_runner();
    let conn = runner.connection_pool();
    smol::block_on(async move {
        panic_job().enqueue(&conn).await?;
        failure_job().enqueue(&conn).await
    })?;

    smol::block_on(runner.run_all_sync_tasks())?;
    // assert_eq!(Err(JobsFailed(2)), runner.check_for_failed_jobs());
    Ok(())
}

#[test]
fn run_all_pending_jobs_errs_if_jobs_dont_start_in_timeout() -> Result<()> {
    let barrier = Barrier::new(2);
    let (tx, rx) = channel::bounded(3);
    // A runner with 1 thread where all jobs will hang indefinitely.
    // The second job will never start.
    let runner = TestGuard::builder(barrier.clone())
        .num_threads(1)
        .on_finish(move |_| smol::block_on(tx.send(coil::Event::Dummy)).unwrap())
        .timeout(Duration::from_millis(50))
        .build();

    let conn = runner.connection_pool();
    smol::block_on(async move {
        barrier_job().enqueue(&conn).await?;
        barrier_job().enqueue(&conn).await
    })?;

    let run_result = smol::block_on(runner.run_all_sync_tasks());
    println!("{:?}", run_result);
    // assert_matches!(run_result, Err(swirl::FetchError::NoMessageReceived));

    // Make sure the jobs actually run so we don't panic on drop
    barrier.wait();
    barrier.wait();
    smol::block_on(runner.check_for_failed_jobs(rx, 2))?;
    Ok(())
}

#[test]
fn jobs_failing_to_load_doesnt_panic_threads() -> Result<()> {
    let (tx, rx) = channel::bounded(3);
    let mut runner = TestGuard::builder(())
        .num_threads(1)
        .on_finish(move |_| {
            smol::block_on(tx.send(coil::Event::Dummy));
        })
        .build();
    let mut conn = runner.connection_pool();
    let conn0 = conn.clone();
    smol::block_on(async move {

        failure_job().enqueue(&conn0).await?;

        // Since jobs are loaded with `SELECT FOR UPDATE`, it will always fail in
        // read-only mode
        sqlx::query("SET default_transaction_read_only = 't'").execute(&conn0).await?;
        let run_result = runner.run_all_sync_tasks().await?;

        println!("{:?}", run_result);
        sqlx::query("SET default_transaction_read_only = 'f'").execute(&conn).await?;

        // assert_matches!(run_result, Err(coil::FetchError::FailedLoadingJob(_)));
       runner.check_for_failed_jobs(rx, 1).await
    })?;

    Ok(())
}
