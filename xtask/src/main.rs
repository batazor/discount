//! `cargo xtask`: the developer commands behind the telemetry contract (ADR 13). They need
//! Docker and are not tests, so they live here rather than under `cargo test`.
//!
//!   cargo xtask weaver check                     validate api/telemetry (exit 1 on a violation)
//!   cargo xtask weaver resolve                   the resolved registry, every ref expanded
//!   cargo xtask weaver diff --baseline-registry <vdir>
//!   cargo xtask weaver <registry subcommand> [args]
//!   cargo xtask live-check                       what a running discount-server emits, against
//!                                                the registry; DATABASE_URL switches it to
//!                                                Postgres so the db.* spans are checked too
//!
//! Weaver runs from the `otel/weaver` image (`WEAVER_IMAGE`), the repository mounted read-only
//! at /work so `.weaver.toml` resolves, the upstream semantic conventions cloned once into
//! `WEAVER_CACHE` (default `~/.cache/discount-weaver`).

use std::env;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

const DEFAULT_IMAGE: &str = "otel/weaver:v0.28.0";
const CONTAINER: &str = "discount-live-check";

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("weaver") if args.len() > 1 => weaver(&args[1..]),
        Some("live-check") => live_check(),
        _ => {
            eprintln!("{}", USAGE);
            return ExitCode::from(2);
        }
    };
    match result {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("FAIL: {e}");
            ExitCode::from(1)
        }
    }
}

const USAGE: &str = "usage:
  cargo xtask weaver <registry subcommand> [args]   (check | resolve | diff | search | ...)
  cargo xtask live-check                            (DATABASE_URL=... for the Postgres store)";

type Result<T> = std::result::Result<T, String>;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask sits in the workspace root")
        .to_path_buf()
}

fn cache_dir() -> Result<PathBuf> {
    let dir = match env::var_os("WEAVER_CACHE") {
        Some(d) => PathBuf::from(d),
        None => PathBuf::from(env::var_os("HOME").ok_or("HOME is not set")?)
            .join(".cache")
            .join("discount-weaver"),
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// `docker run` with the repository and the cache mounted, as the host user so the image's
/// non-root `weaver` user can write the mounted cache.
fn docker_run() -> Result<Command> {
    let root = root();
    let cache = cache_dir()?;
    // SAFETY: getuid/getgid have no preconditions and cannot fail.
    let (uid, gid) = unsafe { (libc::getuid(), libc::getgid()) };
    let mut cmd = Command::new("docker");
    cmd.arg("run")
        .args(["-u", &format!("{uid}:{gid}")])
        .args(["-e", "HOME=/home/weaver"])
        .args(["-v", &format!("{}:/work:ro", root.display())])
        .args(["-v", &format!("{}:/home/weaver/.weaver", cache.display())])
        .args(["-w", "/work"]);
    Ok(cmd)
}

fn image() -> String {
    env::var("WEAVER_IMAGE").unwrap_or_else(|_| DEFAULT_IMAGE.to_string())
}

/// `cargo xtask weaver <args>`: one `weaver registry <args>` run, output passed through.
fn weaver(args: &[String]) -> Result<u8> {
    let status = docker_run()?
        .args(["--rm", "-i"])
        .arg(image())
        .args(["--quiet", "registry"])
        .args(args)
        .status()
        .map_err(|e| format!("docker: {e}"))?;
    Ok(status.code().unwrap_or(1) as u8)
}

/// Stops the service and removes the Weaver container however the run ends.
struct Cleanup {
    server: Option<Child>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        if let Some(mut s) = self.server.take() {
            let _ = s.kill();
            let _ = s.wait();
        }
        let _ = Command::new("docker")
            .args(["rm", "-f", CONTAINER])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// `cargo xtask live-check`: Weaver's OTLP listener in Docker, discount-server exporting to
/// it, the smoke traffic, a graceful stop so the exporters flush, then Weaver's verdict.
fn live_check() -> Result<u8> {
    let root = root();
    let otlp_port = env_or("OTLP_PORT", "4327"); // lgtm owns 4317
    let admin_port = env_or("ADMIN_PORT", "4320");
    let listen = env_or("DISCOUNT_LISTEN", "127.0.0.1:8089");
    let service = format!("http://{listen}");
    let admin = format!("http://127.0.0.1:{admin_port}");

    let built = Command::new("cargo")
        .args([
            "build",
            "-p",
            "discount-infrastructure",
            "--bin",
            "discount-server",
        ])
        .current_dir(&root)
        .status()
        .map_err(|e| format!("cargo build: {e}"))?;
    if !built.success() {
        return Err("discount-server did not build".into());
    }

    let mut cleanup = Cleanup { server: None };
    let _ = Command::new("docker")
        .args(["rm", "-f", CONTAINER])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    run_ok(
        docker_run()?
            .args(["-d", "--name", CONTAINER])
            .args(["-p", &format!("127.0.0.1:{otlp_port}:4317")])
            .args(["-p", &format!("127.0.0.1:{admin_port}:4320")])
            .arg(image())
            .args(["--quiet", "registry", "live-check"])
            .stdout(Stdio::null()),
        "start weaver",
    )?;
    wait_for(&format!("{admin}/health"), "weaver live-check")?;
    pass(&format!(
        "weaver listening for OTLP on 127.0.0.1:{otlp_port}"
    ));

    let log: Stdio = match env::var_os("LIVE_CHECK_SERVER_LOG") {
        Some(p) => File::create(&p)
            .map(Stdio::from)
            .map_err(|e| format!("open {}: {e}", Path::new(&p).display()))?,
        None => Stdio::null(),
    };
    let server = Command::new(root.join("target/debug/discount-server"))
        .env(
            "OTEL_EXPORTER_OTLP_ENDPOINT",
            format!("http://127.0.0.1:{otlp_port}"),
        )
        .env("OTEL_METRIC_EXPORT_INTERVAL", "1000")
        .env("DISCOUNT_LISTEN", &listen)
        .env("DISCOUNT_ENV", "live-check")
        // The traffic below carries no bearer token (ADR 12: `off` is the local mode).
        .env("DISCOUNT_AUTH", "off")
        .env("DISCOUNT_PROMOTIONS", root.join("ops/promotions.json"))
        .env(
            "RUST_LOG",
            env_or(
                "RUST_LOG",
                "info,opentelemetry=warn,h2=warn,tonic=warn,hyper=warn",
            ),
        )
        .stdout(Stdio::null())
        .stderr(log)
        .spawn()
        .map_err(|e| format!("start discount-server: {e}"))?;
    cleanup.server = Some(server);
    wait_for(&format!("{service}/healthz"), "discount-server")?;
    pass(&format!("discount-server on {listen} exporting to weaver"));

    send_traffic(&service)?;
    pass("traffic sent: 3 evaluations, 1 rejection, redeem/idempotent/confirm/409/cancel/404");

    // The budget gauge is refreshed from the catalogue every 10 seconds; wait for one
    // refresh after the PUT so `discount.budget.remaining` is in the sample. Then SIGINT:
    // the graceful shutdown flushes the span, metric and log exporters.
    std::thread::sleep(Duration::from_secs(11));
    let mut server = cleanup.server.take().expect("server running");
    // SAFETY: kill(2) on a pid this process spawned and still owns.
    unsafe { libc::kill(server.id() as libc::pid_t, libc::SIGINT) };
    let _ = server.wait();
    pass("discount-server stopped and flushed");

    // Tell Weaver the sample is complete; it renders the report and exits 0 (nothing at or
    // above `fail_on` in .weaver.toml) or 1. If the admin port no longer answers, Weaver
    // has already exited: its log says why, so fall through to it.
    if let Err(e) = http().post(&format!("{admin}/stop")).send_empty() {
        eprintln!("warn: weaver /stop: {e}");
    }
    let code = Command::new("docker")
        .args(["wait", CONTAINER])
        .output()
        .map_err(|e| format!("docker wait: {e}"))?;
    let code: u8 = String::from_utf8_lossy(&code.stdout)
        .trim()
        .parse()
        .map_err(|_| "docker wait: no exit code".to_string())?;
    let _ = Command::new("docker").args(["logs", CONTAINER]).status();
    if code != 0 {
        return Err(format!("live-check reported violations (exit {code})"));
    }
    pass("live-check: the emitted telemetry matches api/telemetry");
    Ok(0)
}

/// The same traffic as ops/smoke.sh: catalogue, valid and invalid carts, a redemption round
/// trip with an idempotent repeat and a rejected second confirm, a confirm of a missing order.
fn send_traffic(service: &str) -> Result<()> {
    let customer = json!({"customer_id": "live-check", "completed_orders": 0, "uses": [], "attributes": [], "allowlisted": []});
    let cart = json!({"currency": "EUR", "lines": [
        {"line_id": "L1", "unit_price": {"amount": 1999, "currency": "EUR"}, "quantity": 2},
        {"line_id": "L2", "unit_price": {"amount": 501, "currency": "EUR"}, "quantity": 1}],
        "tags": ["#ios"], "attributes": [], "coupon_codes": []});
    let bad_cart =
        json!({"currency": "eur", "lines": [], "tags": [], "attributes": [], "coupon_codes": []});
    let order = format!(
        "live-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    );

    let expect = |what: &str, got: u16, want: u16| -> Result<()> {
        if got == want {
            Ok(())
        } else {
            Err(format!("{what}: expected {want}, got {got}"))
        }
    };
    let put = |path: &str, body: Value| -> Result<u16> { call("PUT", service, path, Some(body)) };
    let post =
        |path: &str, body: Option<Value>| -> Result<u16> { call("POST", service, path, body) };

    expect(
        "put group",
        put(
            "/v1/groups/g",
            json!({"group_id": "g", "priority": 0, "mode": {"type": "Stackable"}}),
        )?,
        200,
    )?;
    expect(
        "put P1",
        put(
            "/v1/promotions/P1",
            json!({"promotion_id": "P1", "group_id": "g", "priority": 10, "exclusive": false, "cap": null, "remaining_budget": null, "uses_total": 0, "rounding": null, "rules": [], "action": {"type": "OrderPercentage", "bps": 1000}}),
        )?,
        200,
    )?;
    expect(
        "put P2",
        put(
            "/v1/promotions/P2",
            json!({"promotion_id": "P2", "group_id": "g", "priority": 5, "exclusive": false, "cap": {"amount": 300, "currency": "EUR"}, "remaining_budget": null, "uses_total": 0, "rounding": null, "rules": [], "action": {"type": "OrderFixedAmount", "amount": {"amount": 500, "currency": "EUR"}}}),
        )?,
        200,
    )?;
    expect(
        "put budget",
        put(
            "/v1/promotions/P1/budget",
            json!({"remaining": {"amount": 1000000, "currency": "EUR"}}),
        )?,
        200,
    )?;

    let evaluate = json!({"cart": cart, "customer": customer});
    for _ in 0..3 {
        expect(
            "evaluate",
            post("/v1/evaluate", Some(evaluate.clone()))?,
            200,
        )?;
    }
    expect(
        "invalid cart",
        post(
            "/v1/evaluate",
            Some(json!({"cart": bad_cart, "customer": customer})),
        )?,
        422,
    )?;

    let mut redeem = evaluate.clone();
    redeem["order_id"] = json!(order);
    expect("redeem", post("/v1/redeem", Some(redeem.clone()))?, 200)?;
    expect("idempotent redeem", post("/v1/redeem", Some(redeem))?, 200)?;
    expect(
        "confirm",
        post(&format!("/v1/redemptions/{order}/confirm"), None)?,
        200,
    )?;
    expect(
        "second confirm",
        post(&format!("/v1/redemptions/{order}/confirm"), None)?,
        409,
    )?;
    expect(
        "cancel",
        post(&format!("/v1/redemptions/{order}/cancel"), None)?,
        200,
    )?;
    expect(
        "confirm of a missing order",
        post("/v1/redemptions/no-such-order/confirm", None)?,
        404,
    )?;
    Ok(())
}

fn http() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into()
}

fn call(method: &str, service: &str, path: &str, body: Option<Value>) -> Result<u16> {
    let url = format!("{service}{path}");
    let agent = http();
    let response = match (method, body) {
        ("PUT", Some(b)) => agent.put(&url).send_json(&b),
        ("POST", Some(b)) => agent.post(&url).send_json(&b),
        ("POST", None) => agent.post(&url).send_empty(),
        _ => unreachable!("only PUT/POST with or without a body"),
    };
    response
        .map(|r| r.status().as_u16())
        .map_err(|e| format!("{method} {path}: {e}"))
}

fn wait_for(url: &str, what: &str) -> Result<()> {
    let agent = http();
    let deadline = Instant::now() + Duration::from_secs(30);
    while Instant::now() < deadline {
        if agent.get(url).call().is_ok_and(|r| r.status().is_success()) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    Err(format!("{what} did not come up ({url})"))
}

fn run_ok(cmd: &mut Command, what: &str) -> Result<()> {
    let status = cmd.status().map_err(|e| format!("{what}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{what}: exit {status}"))
    }
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn pass(msg: &str) {
    println!("ok:   {msg}");
}
