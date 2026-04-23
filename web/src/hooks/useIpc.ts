import { useState, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { IpcError } from '../types/generated';

// useIpc 返回值接口
interface UseIpcReturn {
  loading: boolean;
  error: IpcError | null;
  call: <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
  clearError: () => void;
}

// 统一封装 Tauri IPC 调用，管理 loading/error 状态
export function useIpc(): UseIpcReturn {
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<IpcError | null>(null);

  const call = useCallback(async <T>(cmd: string, args?: Record<string, unknown>): Promise<T> => {
    setLoading(true);
    setError(null);
    try {
      const result = await invoke<T>(cmd, args);
      return result;
    } catch (err) {
      const ipcError = err as IpcError;
      setError(ipcError);
      throw ipcError;
    } finally {
      setLoading(false);
    }
  }, []);

  const clearError = useCallback(() => setError(null), []);
  return { loading, error, call, clearError };
}
