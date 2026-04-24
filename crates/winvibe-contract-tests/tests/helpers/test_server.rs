use std::sync::Arc;
use winvibe_core::approval::types::ApprovalStoreLimits;
use winvibe_core::clock::{FakeMonotonicClock, FakeWallClock};
use winvibe_hook_server::runtime::ApprovalRuntime;
use winvibe_hook_server::server::ServerHandle;
use winvibe_hook_server::sink::NoopSink;

/// 契约测试用 HTTP 服务器封装
pub struct TestServer {
    pub runtime: Arc<ApprovalRuntime>,
    handle: Arc<ServerHandle>,
    token: String,
}

impl TestServer {
    /// 启动测试服务器，绑定随机端口
    pub async fn start() -> Self {
        Self::start_with_token("test-token-contract").await
    }

    /// 使用指定 token 启动测试服务器
    pub async fn start_with_token(token: &str) -> Self {
        let wall = Arc::new(FakeWallClock::default());
        let mono = Arc::new(FakeMonotonicClock::new(10_000));
        let sink: Arc<NoopSink> = Arc::new(NoopSink);
        let runtime = Arc::new(ApprovalRuntime::new(
            ApprovalStoreLimits::default(),
            wall,
            mono,
            sink,
            300_000,
        ));
        let handle = ServerHandle::start("127.0.0.1:0", runtime.clone(), token.to_owned())
            .await
            .expect("启动测试服务器失败");
        Self {
            runtime,
            handle,
            token: token.to_owned(),
        }
    }

    /// 返回服务器基础 URL，如 http://127.0.0.1:12345
    pub fn base_url(&self) -> String {
        let addr = self.handle.local_addr();
        format!("http://{addr}")
    }

    /// 返回 Bearer 认证头值
    pub fn bearer(&self) -> String {
        format!("Bearer {}", self.token)
    }

    /// 返回原始 auth token（不含 "Bearer " 前缀）
    pub fn auth_token(&self) -> &str {
        &self.token
    }

    /// 优雅关闭服务器
    pub async fn shutdown(&self) {
        let _ = self.handle.shutdown().await;
    }
}
