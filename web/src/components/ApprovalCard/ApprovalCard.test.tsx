import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ApprovalCard } from './ApprovalCard';
import type { RedactedApproval } from '../../types/generated';

// 测试用的 pending 审批数据
const pendingApproval: RedactedApproval = {
  id: 'card-test-id',
  session_hash: 'abcd1234',
  tool_name: 'Bash',
  fingerprint: 'b'.repeat(64),
  fingerprint_version: 1,
  state: 'pending',
  decision_kind: null,
  feedback: null,
  created_wall: '2026-04-23T10:00:00Z',
};

// 测试用的 decided 审批数据
const decidedApproval: RedactedApproval = {
  ...pendingApproval,
  id: 'card-decided-id',
  state: 'decided',
  decision_kind: 'Approved',
};

describe('ApprovalCard', () => {
  it('渲染工具名称', () => {
    render(<ApprovalCard approval={pendingApproval} onApprove={vi.fn()} onDeny={vi.fn()} />);
    expect(screen.getByText('Bash')).toBeDefined();
  });

  it('pending 状态显示操作按钮', () => {
    render(<ApprovalCard approval={pendingApproval} onApprove={vi.fn()} onDeny={vi.fn()} />);
    expect(screen.getByText('Approve')).toBeDefined();
    expect(screen.getByText('Deny')).toBeDefined();
  });

  it('decided 状态不显示操作按钮', () => {
    render(<ApprovalCard approval={decidedApproval} onApprove={vi.fn()} onDeny={vi.fn()} />);
    expect(screen.queryByText('Approve')).toBeNull();
    expect(screen.queryByText('Deny')).toBeNull();
  });

  it('点击 Approve 传递正确 id', () => {
    const onApprove = vi.fn();
    render(<ApprovalCard approval={pendingApproval} onApprove={onApprove} onDeny={vi.fn()} />);
    fireEvent.click(screen.getByText('Approve'));
    expect(onApprove).toHaveBeenCalledWith('card-test-id');
  });

  it('点击 Deny 传递正确 id', () => {
    const onDeny = vi.fn();
    render(<ApprovalCard approval={pendingApproval} onApprove={vi.fn()} onDeny={onDeny} />);
    fireEvent.click(screen.getByText('Deny'));
    expect(onDeny).toHaveBeenCalledWith('card-test-id');
  });

  it('loading=true 时按钮禁用', () => {
    render(<ApprovalCard approval={pendingApproval} onApprove={vi.fn()} onDeny={vi.fn()} loading />);
    const approveBtn = screen.getByText('Approve') as HTMLButtonElement;
    expect(approveBtn.disabled).toBe(true);
  });

  it('显示会话哈希', () => {
    render(<ApprovalCard approval={pendingApproval} onApprove={vi.fn()} onDeny={vi.fn()} />);
    expect(screen.getByText('abcd1234')).toBeDefined();
  });
});
