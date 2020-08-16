<div align="center">

# Coil
An async task queue built with SQLx, Postgres, and Rayon

![](https://github.com/insipx/coil/workflows/Unit%20and%20Integration%20Tests/badge.svg)
[![Code Coverage](https://coveralls.io/repos/github/insipx/coil/badge.svg?branch=master)](https://coveralls.io/github/insipx/coil?branch=master)

</div>

Coil is built first for use in [`substrate-archive`](https://github.com/paritytech/substrate-archive). It is heavily inspired by and takes heavily from [swirl](https://github.com/sgrif/swirl). In many places of the codebase, code is very similiar.

Supports synchronous and asynchronous jobs. Synchronous jobs will be spawned into a threadpool managed by [`rayon`](https://github.com/rayon-rs/rayon). Async jobs will be spawned onto an executor. The only requirement is that the executor implements the futures `Spawn` trait. This way, `coil` supports `Tokio`, `smol`, and `async-std`.

<sub><sup>† This software is alpha, and not intended for production use yet. Use at your own risk.

<sub><sup>†† This software is sublicensed as GPLv3. Portions from `swirl` are licensed under MIT. See the [License](#license) section

---

### Examples

```rust
struct Size {
	width: u32,
	height: u32
}

#[coil::background_task]
async fn resize_image(id: u32, size: Size) -> Result<(), Error> {
	// some work
}
```

With an environment
```rust
struct Size {
	width: u32,
	height: u32
}

struct Environment {
    file_server_private_key: String,
    http_client: http_lib::Client,
    conn: sqlx::PgPool
}

#[coil::background_task]
async fn resize_image(env: &Environment, id: u32, size: Size) -> Result<(), Error> {
	// some work
}
```

```rust
resize_image_with_env("tohru".to_string(), Size { height: 32, width: 32 }).enqueue(&pool).await;
let runner = coil::RunnerBuilder::new(env, Executor, pool)
    .num_threads(8)
    .build()
    .unwrap();
runner.run_all_pending_tasks().await.unwrap()
```

### Differences from [`swirl`](https://github.com/sgrif/swirl)
- Supports asynchronous jobs/executors
- Supports jobs with generic arguments
- Serializes data into Postgres with Messagepack instead of JSON
- In asynchronous jobs, database queries will be run asynchronously with SQLx
- Migrations are included in the binary and exposed via a `migrate` fn. 
- Enqueue is an `async fn`

## License
This program includes code from the `Swirl` library, used under the [MIT License](https://github.com/sgrif/swirl/blob/master/LICENSE-MIT) or [https://opensource.org/licenses/MIT](https://opensource.org/licenses/MIT)

This program is sublicensed under [GPLv3](https://github.com/insipx/coil/blob/master/LICENSE). An original MIT license copy for `Swirl` is provided in the source.

