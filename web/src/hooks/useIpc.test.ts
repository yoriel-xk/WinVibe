import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useIpc } from './useIpc';

// 模拟 Tauri IPC 模块
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

import { invoke } from '@tauri-apps/api/core';

describe('useIpc', () => {
  beforeEach(() => { vi.clearAllMocks(); });

  it('初始状态为空闲', () => {
    const { result } = renderHook(() => useIpc());
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('调用时设置 loading，成功后清除', async () => {
    (invoke as ReturnType<typeof vi.fn>).mockResolvedValueOnce({ active: null, cached: [], revision: 0 });
    const { result } = renderHook(() => useIpc());
    let promise: Promise<unknown>;
    act(() => { promise = result.current.call('snapshot'); });
    expect(result.current.loading).toBe(true);
    await act(async () => { await promise!; });
    expect(result.current.loading).toBe(false);
    expect(result.current.error).toBeNull();
  });

  it('调用失败时捕获错误', async () => {
    (invoke as ReturnType<typeof vi.fn>).mockRejectedValueOnce({ code: 'ipc_internal', message: 'oops' });
    const { result } = renderHook(() => useIpc());
    await act(async () => {
      try { await result.current.call('snapshot'); } catch { /* 预期异常 */ }
    });
    expect(result.current.error).toEqual({ code: 'ipc_internal', message: 'oops' });
  });

  it('clearError 清除错误状态', async () => {
    (invoke as ReturnType<typeof vi.fn>).mockRejectedValueOnce({ code: 'err', message: 'fail' });
    const { result } = renderHook(() => useIpc());
    await act(async () => {
      try { await result.current.call('snapshot'); } catch { /* 预期异常 */ }
    });
    expect(result.current.error).not.toBeNull();
    act(() => { result.current.clearError(); });
    expect(result.current.error).toBeNull();
  });
});
