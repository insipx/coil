# Coil
An async task queue built with SQLx, Postgres, and Rayon


##### This software is alpha, and not intended for production use yet

Coil is built first for use in [`substrate-archive`](https://github.com/paritytech/substrate-archive) and takes heavily from [swirl](https://github.com/sgrif/swirl)



Supports Synchronous and Asynchronous jobs. Synchronous jobs will be spawned into a threadpool managed by [`rayon`](https://github.com/rayon-rs/rayon). Async jobs will be spawned onto an executor. The only requirement is that the executor implements the futures `Spawn` trait. This way, `coil` supports `Tokio`, `smol`, and `async-std`.



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
