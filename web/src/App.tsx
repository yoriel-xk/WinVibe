import { useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ApprovalProvider, useApproval } from './context/ApprovalContext';
import { ApprovalCard } from './components/ApprovalCard/ApprovalCard';
import { useIpc } from './hooks/useIpc';
import type { RedactedSnapshot, HudDecision, IpcEvent } from './types/generated';
import styles from './App.module.css';

// 主内容组件，依赖 ApprovalProvider
function AppContent() {
  const { state, dispatch } = useApproval();
  const { loading, call } = useIpc();

  // 拉取最新快照并更新状态
  const fetchSnapshot = useCallback(async () => {
    try {
      const snapshot = await call<RedactedSnapshot>('snapshot');
      dispatch({ type: 'SNAPSHOT_UPDATED', payload: snapshot });
    } catch { /* 错误由 useIpc 管理 */ }
  }, [call, dispatch]);

  // 初始化时拉取快照，并订阅 IPC 事件
  useEffect(() => {
    fetchSnapshot();
    const unlistenPushed = listen<IpcEvent>('approval_pushed', () => fetchSnapshot());
    const unlistenResolved = listen<IpcEvent>('approval_resolved', () => fetchSnapshot());
    return () => {
      unlistenPushed.then(fn => fn());
      unlistenResolved.then(fn => fn());
    };
  }, [fetchSnapshot]);

  // 批准操作：乐观更新 + IPC 调用，失败时回滚
  const handleApprove = useCallback(async (id: string) => {
    dispatch({ type: 'OPTIMISTIC_DECIDE', payload: { id, decision_kind: 'Approved' } });
    try {
      await call('decide', { id, decision: { kind: 'Approved' } as HudDecision });
    } catch {
      dispatch({ type: 'DECIDE_FAILED', payload: { id } });
    }
  }, [call, dispatch]);

  // 拒绝操作：乐观更新 + IPC 调用，失败时回滚
  const handleDeny = useCallback(async (id: string) => {
    dispatch({ type: 'OPTIMISTIC_DECIDE', payload: { id, decision_kind: 'Denied' } });
    try {
      await call('decide', { id, decision: { kind: 'Denied' } as HudDecision });
    } catch {
      dispatch({ type: 'DECIDE_FAILED', payload: { id } });
    }
  }, [call, dispatch]);

  return (
    <div className={styles.container}>
      <h1 className={styles.title}>WinVibe</h1>
      {state.active ? (
        <ApprovalCard
          approval={state.active}
          onApprove={handleApprove}
          onDeny={handleDeny}
          loading={loading}
        />
      ) : (
        <div className={styles.waiting}>Waiting for approval requests...</div>
      )}
      {state.cached.length > 0 && (
        <div className={styles.history}>
          <h2 className={styles.historyTitle}>History</h2>
          {state.cached.map(a => (
            <ApprovalCard key={a.id} approval={a} onApprove={handleApprove} onDeny={handleDeny} />
          ))}
        </div>
      )}
    </div>
  );
}

// 根组件，提供 ApprovalProvider 上下文
export default function App() {
  return (
    <ApprovalProvider>
      <AppContent />
    </ApprovalProvider>
  );
}
