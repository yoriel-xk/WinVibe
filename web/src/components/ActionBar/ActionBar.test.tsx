import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ActionBar } from './ActionBar';

describe('ActionBar', () => {
  it('渲染 Approve 和 Deny 按钮', () => {
    render(<ActionBar onApprove={vi.fn()} onDeny={vi.fn()} />);
    expect(screen.getByText('Approve')).toBeDefined();
    expect(screen.getByText('Deny')).toBeDefined();
  });

  it('点击 Approve 触发回调', () => {
    const onApprove = vi.fn();
    render(<ActionBar onApprove={onApprove} onDeny={vi.fn()} />);
    fireEvent.click(screen.getByText('Approve'));
    expect(onApprove).toHaveBeenCalledOnce();
  });

  it('点击 Deny 触发回调', () => {
    const onDeny = vi.fn();
    render(<ActionBar onApprove={vi.fn()} onDeny={onDeny} />);
    fireEvent.click(screen.getByText('Deny'));
    expect(onDeny).toHaveBeenCalledOnce();
  });

  it('disabled 时按钮不可点击', () => {
    const onApprove = vi.fn();
    const onDeny = vi.fn();
    render(<ActionBar onApprove={onApprove} onDeny={onDeny} disabled />);
    const approveBtn = screen.getByText('Approve') as HTMLButtonElement;
    const denyBtn = screen.getByText('Deny') as HTMLButtonElement;
    expect(approveBtn.disabled).toBe(true);
    expect(denyBtn.disabled).toBe(true);
  });
});
