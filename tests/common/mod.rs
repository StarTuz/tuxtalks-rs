pub mod mock_asr;
pub mod mock_tts;

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use std::{env, fs, thread};
use tempfile::TempDir;

pub struct TestContext {
    pub temp_dir: TempDir,
    pub child: Child,
    pub socket_path: PathBuf,
}

impl TestContext {
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let bin_path = env!("CARGO_BIN_EXE_tuxtalks");

        // Setup env vars to isolate the daemon
        // We use the temp_dir as config/data/runtime dir
        let runtime_dir = temp_dir.path().join("runtime");
        let config_dir = temp_dir.path().join("config");
        let data_dir = temp_dir.path().join("data");

        fs::create_dir_all(&runtime_dir).expect("Failed to create runtime dir");
        fs::create_dir_all(&config_dir).expect("Failed to create config dir");
        fs::create_dir_all(&data_dir).expect("Failed to create data dir");

        // Generate a unique user name for this test run to isolate socket paths
        // This avoids conflicts between parallel tests and stale sockets from killed processes.
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos();
        let test_user = format!("tuxtalks_test_{}", nanos);

        // Spawn the daemon
        let child = Command::new(bin_path)
            .env("XDG_RUNTIME_DIR", &runtime_dir)
            .env("XDG_CONFIG_HOME", &config_dir)
            .env("XDG_DATA_HOME", &data_dir)
            .env("USER", &test_user) // Force daemon to use our unique socket path
            // Disable other env vars that might interfere
            .env_remove("TUXTALKS_CONFIG")
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Failed to spawn tuxtalks daemon");

        // Calculate expected socket path
        let socket_path = PathBuf::from(format!("/tmp/tuxtalks-{}.sock", test_user));

        let ctx = TestContext {
            temp_dir,
            child,
            socket_path,
        };

        ctx.wait_for_socket();
        ctx
    }

    fn wait_for_socket(&self) {
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(5) {
            if self.socket_path.exists() {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("Timed out waiting for socket at {:?}", self.socket_path);
    }
}

impl Drop for TestContext {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
