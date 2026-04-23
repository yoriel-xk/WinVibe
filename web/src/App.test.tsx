import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import App from './App';

// 模拟 Tauri IPC 模块
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({ active: null, cached: [], revision: 0 }),
}));

// 模拟 Tauri 事件模块
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe('App', () => {
  it('无活跃审批时显示等待提示', async () => {
    render(<App />);
    expect(await screen.findByText(/waiting/i)).toBeDefined();
  });
});
