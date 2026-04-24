use crate::handlers::build_router;
use crate::runtime::ApprovalRuntime;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// 服务器内部状态，shutdown 后被 take
struct ServerHandleInner {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    join: tokio::task::JoinHandle<()>,
    local_addr: SocketAddr,
}

/// HTTP 服务器句柄，支持优雅关闭
pub struct ServerHandle {
    inner: Mutex<Option<ServerHandleInner>>,
    shutting_down: AtomicBool,
}

/// 关闭错误
#[derive(Debug, thiserror::Error)]
pub enum ShutdownError {
    #[error("already shutting down")]
    AlreadyShuttingDown,
}

impl ServerHandle {
    /// 启动 HTTP 服务器，绑定到指定地址
    pub async fn start(
        bind: &str,
        runtime: Arc<ApprovalRuntime>,
        auth_token: String,
    ) -> Result<Arc<Self>, std::io::Error> {
        let app = build_router(runtime, auth_token);
        let listener = tokio::net::TcpListener::bind(bind).await?;
        let local_addr = listener.local_addr()?;

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel();

        let join = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .ok();
        });

        Ok(Arc::new(Self {
            inner: Mutex::new(Some(ServerHandleInner {
                shutdown_tx,
                join,
                local_addr,
            })),
            shutting_down: AtomicBool::new(false),
        }))
    }

    /// 获取实际绑定的本地地址（端口 0 时可获取分配的端口）
    pub fn local_addr(&self) -> SocketAddr {
        self.inner.try_lock().unwrap().as_ref().unwrap().local_addr
    }

    /// 优雅关闭服务器，重复调用返回错误
    pub async fn shutdown(&self) -> Result<(), ShutdownError> {
        if self.shutting_down.swap(true, Ordering::SeqCst) {
            return Err(ShutdownError::AlreadyShuttingDown);
        }
        let inner = self
            .inner
            .lock()
            .await
            .take()
            .ok_or(ShutdownError::AlreadyShuttingDown)?;
        let _ = inner.shutdown_tx.send(());
        let _ = inner.join.await;
        Ok(())
    }
}

#[cfg(test)]
fn test_runtime() -> Arc<ApprovalRuntime> {
    use crate::sink::NoopSink;
    use winvibe_core::approval::types::ApprovalStoreLimits;
    use winvibe_core::clock::{FakeMonotonicClock, FakeWallClock};
    Arc::new(ApprovalRuntime::new(
        ApprovalStoreLimits::default(),
        Arc::new(FakeWallClock::default()),
        Arc::new(FakeMonotonicClock::new(10_000)),
        Arc::new(NoopSink),
        300_000,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn start_and_shutdown() {
        let handle = ServerHandle::start("127.0.0.1:0", test_runtime(), "test-token".into())
            .await
            .unwrap();
        let addr = handle.local_addr();
        assert_ne!(addr.port(), 0);
        handle.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn double_shutdown_returns_error() {
        let handle = ServerHandle::start("127.0.0.1:0", test_runtime(), "test-token".into())
            .await
            .unwrap();
        handle.shutdown().await.unwrap();
        let result = handle.shutdown().await;
        assert!(result.is_err());
    }
}
