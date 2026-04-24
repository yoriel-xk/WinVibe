pub mod app_state;
pub mod audit;
pub mod commands;
pub mod config_loader;
pub mod diagnostic;
pub mod hud_decision;
pub mod ipc_error;
pub mod lifecycle_sink;
pub mod redact;

/// 关闭时所需的句柄，通过 Tauri state 管理
#[cfg(not(test))]
struct ShutdownState {
    runtime: std::sync::Arc<winvibe_hook_server::runtime::ApprovalRuntime>,
    server_handle: std::sync::Arc<winvibe_hook_server::server::ServerHandle>,
    audit_sink: std::sync::Arc<dyn audit::AuditSink>,
}

/// 应用入口：加载配置、初始化各子系统、启动 Tauri 事件循环
#[cfg(not(test))]
pub fn run() {
    use std::sync::Arc;
    use tauri::Manager;
    // 1. 加载配置（同步，无需 tokio）
    let config_path = config_loader::resolve_config_path_app(None);
    let config = match config_loader::load_or_init_config_app(&config_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("winvibe: config error: {e}");
            std::process::exit(78);
        }
    };

    // 2. 数据目录
    let data_dir = std::env::var("LOCALAPPDATA")
        .map(|d| std::path::PathBuf::from(d).join("WinVibe"))
        .unwrap_or_else(|_| std::path::PathBuf::from(".winvibe"));

    // 3. DiagnosticSink（同步，可在 tokio 外创建）
    let diag_sink = Arc::new(diagnostic::DiagnosticSink::new(
        data_dir.join("diagnostics"),
        true,
    ));

    // 保存配置值供 setup 闭包使用
    let bind_addr = format!("{}:{}", config.bind, config.port);
    let auth_token = config.auth_token.as_str().to_string();
    let ttl_ms = config.approval_ttl_ms;
    let max_cached = config.max_cached;
    let audit_dir = data_dir.join("audit");
    let diag_dir = data_dir.join("diagnostics");

    let diag_sink_clone = Arc::clone(&diag_sink);

    tauri::Builder::default()
        .setup(move |app| {
            let handle = app.handle().clone();

            // JsonlAuditSink 需要 tokio runtime（内部 spawn）
            let audit_sink: Arc<dyn audit::AuditSink> =
                Arc::new(audit::JsonlAuditSink::new(audit_dir.clone()));

            // 启动时清理旧文件（后台线程，不阻塞启动）
            let audit_dir_cleanup = audit_dir.clone();
            let diag_dir_cleanup = diag_dir.clone();
            std::thread::spawn(move || {
                audit::cleanup::cleanup_old_audit_files(
                    &audit_dir_cleanup,
                    std::time::Duration::from_secs(30 * 24 * 3600),
                );
                diagnostic::cleanup::cleanup_old_diagnostic_files(
                    &diag_dir_cleanup,
                    std::time::Duration::from_secs(7 * 24 * 3600),
                );
            });

            // 构造 lifecycle sink（带 Tauri IPC 推送，仅非测试编译可用）
            let lifecycle_sink: Arc<dyn winvibe_hook_server::sink::ApprovalLifecycleSink> =
                Arc::new(lifecycle_sink::AppLifecycleSink::new(
                    Some(Arc::new(lifecycle_sink::TauriIpcEmitter::new(handle))),
                    Arc::clone(&audit_sink),
                    Arc::clone(&diag_sink_clone),
                ));

            // 构造时钟和 runtime
            let wall: Arc<dyn winvibe_core::clock::WallClock> =
                Arc::new(winvibe_core::clock::RealWallClock);
            let mono: Arc<dyn winvibe_core::clock::MonotonicClock> =
                Arc::new(winvibe_core::clock::RealMonotonicClock::new());
            let limits = winvibe_core::approval::types::ApprovalStoreLimits {
                max_active: 1,
                max_cached,
            };
            let runtime = Arc::new(winvibe_hook_server::runtime::ApprovalRuntime::new(
                limits,
                wall,
                mono,
                lifecycle_sink,
                ttl_ms,
            ));

            // 启动 HTTP server（block_on 在 setup 中是安全的）
            let runtime_for_server = Arc::clone(&runtime);
            let server_handle = tauri::async_runtime::block_on(async {
                winvibe_hook_server::server::ServerHandle::start(
                    &bind_addr,
                    runtime_for_server,
                    auth_token,
                )
                .await
            })
            .expect("HTTP server 启动失败");

            // 注册 AppState（供 IPC 命令使用）
            app.manage(app_state::AppState {
                runtime: Arc::clone(&runtime),
                audit_sink: Arc::clone(&audit_sink),
            });

            // 注册 ShutdownState（供关闭事件使用）
            app.manage(ShutdownState {
                runtime,
                server_handle,
                audit_sink,
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::snapshot,
            commands::decide,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // 阻止默认关闭，改为异步执行 §3.5 关闭序列
                api.prevent_close();
                let app_handle = window.app_handle().clone();
                let window = window.clone();
                tauri::async_runtime::spawn(async move {
                    let state = app_handle.state::<ShutdownState>();

                    // §3.5 关闭序列
                    // 步骤 1：阻断新 Pending 请求
                    state.runtime.begin_shutdown();

                    // 步骤 2：取消所有 Pending 审批
                    let trace = winvibe_core::trace::TraceCtx::new(
                        winvibe_core::trace::TraceSource::System(
                            winvibe_core::trace::SystemTraceSource::AppExitCancel,
                        ),
                    );
                    state
                        .runtime
                        .cancel_all_pending(trace, winvibe_core::protocol::CancelReason::AppExit)
                        .await;

                    // 步骤 3：停止 HTTP server
                    let _ = state.server_handle.shutdown().await;

                    // 步骤 4：flush + shutdown audit sink
                    let _ = state.audit_sink.flush().await;
                    let _ = state.audit_sink.shutdown().await;

                    // 步骤 5：允许窗口关闭
                    let _ = window.close();
                });
            }
        })
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}
