import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { StatusBadge } from './StatusBadge';

describe('StatusBadge', () => {
  it('pending 状态显示 Pending', () => {
    render(<StatusBadge state="pending" />);
    expect(screen.getByText('Pending')).toBeDefined();
  });

  it('decided + Approved 显示 Approved', () => {
    render(<StatusBadge state="decided" decisionKind="Approved" />);
    expect(screen.getByText('Approved')).toBeDefined();
  });

  it('decided + Denied 显示 Denied', () => {
    render(<StatusBadge state="decided" decisionKind="Denied" />);
    expect(screen.getByText('Denied')).toBeDefined();
  });

  it('decided + null 显示 Unknown', () => {
    render(<StatusBadge state="decided" decisionKind={null} />);
    expect(screen.getByText('Unknown')).toBeDefined();
  });

  it('decided + TimedOut 显示 TimedOut', () => {
    render(<StatusBadge state="decided" decisionKind="TimedOut" />);
    expect(screen.getByText('TimedOut')).toBeDefined();
  });
});
