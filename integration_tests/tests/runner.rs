use assert_matches::assert_matches;
use anyhow::Result;
use std::thread;
use std::time::Duration;
use futures::{future::FutureExt, StreamExt};
use sqlx::Executor;

use crate::dummy_jobs::*;
use crate::sync::Barrier;
use crate::test_guard::TestGuard;

#[test]
fn run_all_pending_jobs_returns_when_all_jobs_enqueued() -> Result<()> {
    crate::initialize();
    let barrier = Barrier::new(3);
    let (runner, _wait_task) = TestGuard::runner(barrier.clone(), 2);
    log::info!("RUNNING `run_all_pending_jobs_returns_when_all_jobs_enqueued`");
    let conn = runner.connection_pool();

    smol::block_on(async {
        barrier_job().enqueue(&conn).await.unwrap();
        barrier_job().enqueue(&conn).await.unwrap();
        runner.run_all_sync_tasks().await.unwrap();

        let queued_job_count = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM _background_tasks")
            .fetch_one(&conn)
            .await.unwrap()
            .0;
        let unlocked_job_count = sqlx::query_as::<_, (i64,)>("SELECT id FROM _background_tasks FOR UPDATE SKIP LOCKED")
            .fetch_all(&conn)
            .await
            .unwrap()
            .len();

        assert_eq!(2, queued_job_count);
        assert_eq!(0, unlocked_job_count);
    });

    barrier.wait();
    Ok(())
}


#[test]
fn check_for_failed_jobs_blocks_until_all_queued_jobs_are_finished() -> Result<()> {
    crate::initialize();
    let barrier = Barrier::new(3);
    let (runner, task_wait) = TestGuard::runner(barrier.clone(), 2);
    log::info!("RUNNING `check_for_failed_jobs_blocks_until_all_queued_jobs_are_finished`");
    let conn = runner.connection_pool();
    smol::block_on(async {
        barrier_job().enqueue(&conn).await?;
        barrier_job().enqueue(&conn).await
    })?;

    smol::block_on(runner.run_all_sync_tasks())?;
    let (tx, rx) = channel::bounded(1);

    let handle = thread::spawn(move || {
        let mut rx0 = rx.clone();
        let timeout = timer::Delay::new(Duration::from_millis(100));

        let res = smol::block_on(async {
            futures::select! {
                msg = rx0.next().fuse() => false,
                _ = timeout.fuse() => true
            }
        });
        assert!(
            res,
            "wait_for_jobs returned before jobs finished"
        );

        barrier.wait();

        assert!(smol::block_on(rx.recv()).is_ok(), "wait_for_jobs didn't return");
    });

    let _ = smol::block_on(runner.check_for_failed_jobs(task_wait, 2));
    smol::block_on(tx.send(()))?;
    handle.join().unwrap();
    Ok(())
}


#[test]
fn check_for_failed_jobs_panics_if_jobs_failed() -> Result<()> {
    crate::initialize();
    let (runner, rx) = TestGuard::dummy_runner();
    log::info!("RUNNING `check_for_failed_jobs_panics_if_jobs_failed`");
    let conn = runner.connection_pool();
    smol::block_on(async {
        failure_job().enqueue(&conn).await?;
        failure_job().enqueue(&conn).await?;
        failure_job().enqueue(&conn).await
    })?;

    smol::block_on(runner.run_all_sync_tasks())?;
    assert_eq!(Err(coil::FailedJobsError::JobsFailed(3)), smol::block_on(runner.check_for_failed_jobs(rx, 3)));
    Ok(())
}

#[test]
fn panicking_jobs_are_caught_and_treated_as_failures() -> Result<()> {
    crate::initialize();
    let (runner, rx) = TestGuard::dummy_runner();
    log::info!("RUNNING `panicking_jobs_are_caught_and_treated_as_failures`");
    let conn = runner.connection_pool();
    smol::block_on(async {
        panic_job().enqueue(&conn).await?;
        failure_job().enqueue(&conn).await
    })?;

    smol::block_on(runner.run_all_sync_tasks())?;
    let failed_jobs = smol::block_on(runner.check_for_failed_jobs(rx, 2));
    assert_eq!(Err(coil::FailedJobsError::JobsFailed(2)), failed_jobs);
    Ok(())
}

#[test]
fn run_all_pending_jobs_errs_if_jobs_dont_start_in_timeout() -> Result<()> {
    crate::initialize();
    let barrier = Barrier::new(2);
    let (tx, rx) = channel::bounded(3);
    // A runner with 1 thread where all jobs will hang indefinitely.
    // The second job will never start.
    let runner = TestGuard::builder(barrier.clone())
        .num_threads(1)
        .max_tasks(1)
        .on_finish(move |_| { let _ = smol::block_on(tx.send(coil::Event::Dummy)); })
        .timeout(Duration::from_millis(50))
        .build();
    log::info!("RUNNING `run_all_pending_jobs_errs_if_jobs_dont_start_in_timeout`");
    let conn = runner.connection_pool();
    smol::block_on(async {
        barrier_job().enqueue(&conn).await?;
        barrier_job().enqueue(&conn).await
    })?;

    let run_result = smol::block_on(runner.run_all_sync_tasks());
    assert_matches!(run_result, Err(coil::FetchError::Timeout));

    // Make sure the jobs actually run so we don't panic on drop
    barrier.wait();
    barrier.wait();
    smol::block_on(runner.check_for_failed_jobs(rx, 2)).unwrap();
    Ok(())
}

use sqlx::Connection;
#[test]
// FIXME: This test should work but postgres isn't recognizing default_transaction_read_only for some reason
// I don't know why.
#[ignore]
fn jobs_failing_to_load_doesnt_panic_threads() -> Result<()> {
    crate::initialize();
    let (tx, rx) = channel::bounded(3);

    let runner = TestGuard::builder(())
        .num_threads(1)
        .max_tasks(1)
        .timeout(std::time::Duration::from_secs(1))
        .on_finish(move |_| {
            let _ = smol::block_on(tx.send(coil::Event::Dummy));
        })
        .build();
    log::info!("RUNNING `jobs_failing_to_load_doesnt_panic_threads`");
    smol::run(async {
        let mut conn = runner.connection_pool().acquire().await.unwrap();
        failure_job().enqueue(&mut conn).await.unwrap();
        // Since jobs are loaded with `SELECT FOR UPDATE`, it will always fail in
        // read-only mode
        conn.execute("SET default_transaction_read_only = on").await.unwrap();
    });

    let run_result = smol::block_on(runner.run_all_sync_tasks());

    smol::run(async {
        let mut conn = runner.connection_pool().acquire().await.unwrap();
        conn.execute("SET default_transaction_read_only = off").await.unwrap();
        assert_matches!(run_result, Err(coil::FetchError::FailedLoadingJob(_)));
    });

    // this test is supposed to time out because the job is never actually 'run'
    // because we can't grab it due to read-only transaction
    smol::block_on(runner.check_for_failed_jobs(rx, 1)).unwrap();

    Ok(())
}
