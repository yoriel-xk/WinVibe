import { describe, it, expect } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import React from 'react';
import { ApprovalProvider, useApproval } from './ApprovalContext';
import type { RedactedApproval } from '../types/generated';

// 测试用的模拟审批数据
const mockApproval: RedactedApproval = {
  id: 'test-id',
  session_hash: 'abcd1234',
  tool_name: 'Bash',
  fingerprint: 'a'.repeat(64),
  fingerprint_version: 1,
  state: 'pending',
  decision_kind: null,
  feedback: null,
  created_wall: '2026-04-23T10:00:00Z',
};

// 包装 Provider 的辅助函数
const wrapper = ({ children }: { children: React.ReactNode }) =>
  React.createElement(ApprovalProvider, null, children);

describe('ApprovalContext', () => {
  it('初始状态为空快照', () => {
    const { result } = renderHook(() => useApproval(), { wrapper });
    expect(result.current.state.active).toBeNull();
    expect(result.current.state.cached).toEqual([]);
    expect(result.current.state.revision).toBe(0);
  });

  it('SNAPSHOT_UPDATED 替换整个状态', () => {
    const { result } = renderHook(() => useApproval(), { wrapper });
    act(() => {
      result.current.dispatch({
        type: 'SNAPSHOT_UPDATED',
        payload: { active: mockApproval, cached: [], revision: 1 },
      });
    });
    expect(result.current.state.active).toEqual(mockApproval);
    expect(result.current.state.revision).toBe(1);
  });

  it('OPTIMISTIC_DECIDE 设置 decision_kind', () => {
    const { result } = renderHook(() => useApproval(), { wrapper });
    // 先设置活跃审批
    act(() => {
      result.current.dispatch({
        type: 'SNAPSHOT_UPDATED',
        payload: { active: mockApproval, cached: [], revision: 1 },
      });
    });
    // 乐观更新
    act(() => {
      result.current.dispatch({
        type: 'OPTIMISTIC_DECIDE',
        payload: { id: 'test-id', decision_kind: 'Approved' },
      });
    });
    expect(result.current.state.active?.decision_kind).toBe('Approved');
    expect(result.current.state.active?.state).toBe('decided');
  });

  it('DECIDE_FAILED 回滚到 pending 状态', () => {
    const { result } = renderHook(() => useApproval(), { wrapper });
    // 先设置活跃审批并乐观更新
    act(() => {
      result.current.dispatch({
        type: 'SNAPSHOT_UPDATED',
        payload: { active: mockApproval, cached: [], revision: 1 },
      });
    });
    act(() => {
      result.current.dispatch({
        type: 'OPTIMISTIC_DECIDE',
        payload: { id: 'test-id', decision_kind: 'Approved' },
      });
    });
    // 失败回滚
    act(() => {
      result.current.dispatch({
        type: 'DECIDE_FAILED',
        payload: { id: 'test-id' },
      });
    });
    expect(result.current.state.active?.decision_kind).toBeNull();
    expect(result.current.state.active?.state).toBe('pending');
  });
});
