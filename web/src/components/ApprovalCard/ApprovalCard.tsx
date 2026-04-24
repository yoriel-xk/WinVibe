import { StatusBadge } from '../StatusBadge/StatusBadge';
import { ActionBar } from '../ActionBar/ActionBar';
import type { RedactedApproval } from '../../types/generated';
import styles from './ApprovalCard.module.css';

// ApprovalCard 组件属性
interface ApprovalCardProps {
  approval: RedactedApproval;
  onApprove: (id: string) => void;
  onDeny: (id: string) => void;
  loading?: boolean;
}

// 审批卡片：展示审批详情并提供操作按钮
export function ApprovalCard({ approval, onApprove, onDeny, loading = false }: ApprovalCardProps) {
  const isPending = approval.state === 'pending';
  return (
    <div className={styles.card}>
      {/* 卡片头部：工具名称 + 状态徽章 */}
      <div className={styles.header}>
        <span className={styles.toolName}>{approval.tool_name}</span>
        <StatusBadge state={approval.state} decisionKind={approval.decision_kind} />
      </div>
      {/* 会话哈希 */}
      <div className={styles.meta}>
        <span className={styles.label}>Session:</span>
        <span className={styles.value}>{approval.session_hash}</span>
      </div>
      {/* 创建时间 */}
      <div className={styles.meta}>
        <span className={styles.label}>Created:</span>
        <span className={styles.value}>{new Date(approval.created_wall).toLocaleTimeString()}</span>
      </div>
      {/* 指纹（替代原始输入，已脱敏） */}
      <div className={styles.meta}>
        <span className={styles.label}>Fingerprint:</span>
        <span className={`${styles.value} ${styles.fingerprint}`}>{approval.fingerprint}</span>
      </div>
      {/* 仅 pending 状态显示操作按钮 */}
      {isPending && (
        <ActionBar
          onApprove={() => onApprove(approval.id)}
          onDeny={() => onDeny(approval.id)}
          disabled={loading}
        />
      )}
    </div>
  );
}
