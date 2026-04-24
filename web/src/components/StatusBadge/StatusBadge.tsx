import styles from './StatusBadge.module.css';

// StatusBadge 组件属性
interface StatusBadgeProps {
  state: 'pending' | 'decided';
  decisionKind?: string | null;
}

// 显示审批状态徽章
export function StatusBadge({ state, decisionKind }: StatusBadgeProps) {
  if (state === 'pending') {
    return <span className={`${styles.badge} ${styles.pending}`}>Pending</span>;
  }
  // 已决策：显示决策类型
  const kind = decisionKind ?? 'Unknown';
  const variantClass = (styles as Record<string, string>)[kind.toLowerCase()] ?? styles.unknown;
  return <span className={`${styles.badge} ${variantClass}`}>{kind}</span>;
}
